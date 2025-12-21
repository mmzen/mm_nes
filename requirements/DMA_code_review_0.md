DMA implementation phases 1 - 3 feedback
----

You’ve got the right *shape* (a central cycle tick + a DMA controller), but the DMA integration here is still carrying instruction-boundary assumptions and a couple of flat-out wrong timing decisions. If you keep this as-is, you’ll get “mostly works” DMA, not “hardware-faithful”.

Here’s the breakdown—what’s correct, what’s broken, and what you need to change.

---

## What’s good (keep it)

### 1) Single-cycle scheduler exists

`step_master_cycle()` is the correct place to arbitrate CPU vs DMA and advance PPU/APU.

### 2) OAM DMA trigger wiring

Using `PpuDma::new_with_dma_signal(self.dma_start_page.clone())` so `$4014` writes just signal and the scheduler executes DMA is the right architecture.

### 3) Tracking “last CPU read address”

You’re *trying* to model the “repeated reads during DMA idle cycles” idea. That’s necessary for DPCM/controller/PPUDATA conflict correctness.

---

## The big problems (these will make your DMA timing wrong)

### Problem A — You run APU *before* CPU each cycle

You explicitly say this is “critical”, but it’s actually a red flag:

```rust
// STEP 1: Advance APU FIRST (before CPU) to detect DMC DMA needs
let (apu_cycles, apu_samples) = self.apu.borrow_mut().run(self.apu_counter.current, 1)?;
```

On real hardware, everything is clocked together. You don’t get to “peek” one component early to decide what should happen “this cycle”. Doing APU-first is effectively time travel.

**Consequence:** you can start DMC DMA one cycle earlier than hardware, and you will shift edge cases (especially those involving $4016/$4017 and $2007 conflicts) in the exact situations that matter.

**Fix:** DMC DMA requests must be generated from the APU *state at the beginning of the CPU cycle*, not after “running APU for this cycle”. The DMA arbiter decides bus ownership for the current cycle based on latched requests, not “what APU decided after being advanced”.

In practice:

* Compute “does DMC need a fetch now?” from APU’s counters **before ticking them**, or
* Tick APU as part of a unified “tick phase” and only allow DMC DMA to begin at the correct later phase (see Problem C).

---

### Problem B — `pending_dmc_dma` is half-implemented, but you never use it

You have this field:

```rust
pending_dmc_dma: Option<(u16, u8)>,  // (address, countdown)
```

…but in STEP 2 you start DMC DMA immediately and never set `pending_dmc_dma`. STEP 3 will almost always do nothing.

That means you’ve got dead logic from the old approach and inconsistent intent.

**Fix:** Either:

* delete `pending_dmc_dma` entirely and implement correct scheduling in `DmaController`, or
* actually use it as the *only* mechanism for DMC scheduling (but then it needs correct rules, not an arbitrary countdown).

Right now it’s a sign you’re mid-migration and the code is lying to you.

---

### Problem C — DMC DMA cycle timing is not being scheduled correctly

This line is the giveaway:

```rust
let cycles = DmaController::calculate_dmc_dma_cycles(
    cpu_is_writing,
    false, // CPU not halted yet
    oam_dma_active,
    oam_dma_on_read,
);
self.dma_controller.start_dmc_dma(dmc_address, cycles, self.last_cpu_read_address);
```

Real DMC DMA isn’t “calculate a total number of cycles and run”. It’s a **sequence**:

* halt cycle (must succeed only on CPU read)
* dummy cycle
* optional alignment cycle
* GET read cycle

And the start time differs for **load** vs **reload**:

* load: GET of the 2nd APU cycle after enabling
* reload: starts on a PUT cycle

Your approach suggests you’re compressing this into a fixed “N cycles” and letting the controller consume them. That’s almost certainly wrong around:

* CPU write cycles delaying the halt
* DMC overlap stealing GET from OAM DMA
* alignment interactions with the GET/PUT phase

**Fix:** DMC DMA must be a state machine, not “N cycles”.

If your `DmaController` is already a state machine internally, then this outer code should not be “calculating cycles” at all. It should only say:

* “DMC fetch requested at address X”
  and the controller should decide when it can halt and when it can read based on phase + CPU read/write.

---

### Problem D — Using `cpu.borrow().is_mid_instruction()` to decide “CPU is writing” is not valid

You do:

```rust
let cpu_is_writing = self.cpu.borrow().is_mid_instruction();
```

That’s wrong in principle. “mid-instruction” is not the same as “this cycle is a write cycle”. Many cycles mid-instruction are reads; some are internal.

DMA halt success depends on whether **the current external bus cycle is a write**. That’s a per-cycle bus property, not an instruction property.

**Fix:** the CPU must expose for the current cycle:

* bus R/W direction (read vs write)
* address
* data_out (if write)

Your `CpuCycleResult` suggests you already track `memory_read` / `memory_write`, but you’re using it only after stepping CPU. DMA needs it **before** selecting the bus master for the cycle.

---

### Problem E — `last_cpu_read_address` is not enough to model repeated reads correctly

You store only the last address when `result.memory_read` is true. But real RDY stall repeats the **current read cycle that was halted**, not “the last read some time ago”.

Edge case: if the CPU’s last cycle before DMA was a write, you must not reuse an old read address. The hardware will keep presenting the halted read cycle once it finally halts (and the halt can’t succeed on a write anyway).

Also, repeated reads aren’t just “address repeats”. The **read itself happens**, causing side effects (PPU regs, controller shift, etc.). Your DMA controller gets an address, but I don’t see here that during DMA no-bus cycles you actually perform the read.

**Fix:** When CPU is stalled and DMA is doing a no-bus cycle:

* the system bus must execute the **CPU repeated read** for that cycle (with all side effects)

That requires storing more than an address; it requires the CPU’s “halted read microcycle” state or at least the last externally visible read bus operation at the moment RDY dropped.

---

### Problem F — You toggle GET/PUT phase after the fact and you patch legacy parity too

This is messy:

```rust
for _ in 0..total_cpu_cycles {
    self.dma_controller.toggle_apu_phase();
}
if total_cpu_cycles % 2 == 1 {
    let new_parity = !self.cpu_cycle_odd.get();
    self.cpu_cycle_odd.set(new_parity);
}
```

Two issues:

1. GET/PUT phase is a property of **each CPU cycle**, but you’re toggling after executing the cycle, based on `total_cpu_cycles` which includes `interrupt_cycles`. That’s dangerous: “interrupt_cycles” here seems like “extra cycles happened”, but you didn’t actually run extra cycles through DMA arbitration, APU tick, PPU tick, etc. You just jumped counters.

2. You’re still maintaining `cpu_cycle_odd` for “backward compatibility”. That’s exactly how you end up with two competing time models.

**Fix:** Remove the concept of “interrupt_cycles” being added as a lump. If an interrupt takes 7 cycles, you must literally execute 7 cycle ticks. Anything else breaks DMA and bus-level interactions.

---

## Summary: the DMA is not actually cycle-accurate yet

You have a cycle loop, but you still “batch” time (`interrupt_cycles`), “peek early” (APU-first), and decide bus behavior using instruction-level concepts (`is_mid_instruction`). Those are instruction-boundary habits disguised inside a per-cycle wrapper.

That’s the worst category: it looks correct and will mislead you for months.

---

## Concrete changes you should demand next

1. **Unify bus ownership decision BEFORE any component advances**

    * Each CPU cycle: decide which master owns the bus (CPU vs OAM DMA vs DMC DMA vs repeated-read)
    * Then perform exactly one bus action
    * Then tick CPU/APU/PPU one cycle

2. **DMC DMA scheduling must move entirely into DmaController**

    * The outer scheduler must not “calculate cycles”
    * It should only pass “DMC fetch requested” events with address

3. **Eliminate “interrupt_cycles” batching**

    * If CPU needs multiple cycles, it must call `step_cycle()` multiple times or expose microcycles explicitly.
    * Your global tick must never “skip cycles”.

4. **Replace `is_mid_instruction()` with per-cycle bus R/W**

    * CPU must output current cycle bus intent (R/W/address/data_out) even when stalled.

5. **Repeated reads must be real reads**

    * During DMA idle cycles, the bus must perform the repeated read cycle with side effects.

If you want, paste `DmaController`, `calculate_dmc_dma_cycles`, and `CpuCycleResult`/`CPU::step_cycle()` signatures. The real verdict depends on whether the controller is genuinely sequencing GET/PUT + halt/dummy/align/read internally, or if it’s doing the common “countdown stall” hack.
