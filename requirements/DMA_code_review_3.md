Alright. With the updated `nes_console.rs` + the rewritten `dma_controller.rs`, you’re closer to a *real* cycle-accurate architecture. But you’re still leaking correctness through a few key seams.

Below is a regenerated code review for **both files together**, focusing on what will break real ROMs and what will make your “cycle accurate” claim false.

---

# Executive verdict

**Good:** You’ve moved to an explicit **bus arbiter** model and you’re trying to align DMA decisions to **pre-cycle CPU intent**, which is the right mental model.

**Bad:** You still have **phase drift risk**, **PPU timing mismatch**, and **DMC correctness is still not proven** because your state machine makes assumptions you haven’t validated. Also, the console scheduler currently violates its own comments about how many PPU “dots” per cycle it runs.

If you ship this as “cycle accurate,” you’ll pass a bunch of tests and then get wrecked by timing ROMs and edge-case game code.

---

# Review: `dma_controller.rs`

## What’s strong

* **One bus op per CPU cycle** contract is explicit and enforced. That’s the core truth source.
* Explicit `BusOp` and `BusWinner` enables deterministic traces.
* OAM PUT goes through `$2004` on the bus (so PPU side effects happen where they should).
* You correctly model that **DMA can have “no-bus” cycles** and you model CPU repeated reads during those cycles.

## What’s still wrong / risky

### 1) DMC DMA completion semantics are almost certainly wrong

You end DMC DMA immediately on the `Read` cycle:

```rust
self.dmc.sample = Some(value);
result.dmc_sample = Some(value);
self.dmc.phase = DmcDmaPhase::Idle;
```

That assumes the entire DMA transaction is: halt/dummy/align/read, then immediately resume CPU.

Hardware behavior is subtle here. The CPU stall length and the exact cycle the sample becomes visible can create off-by-one errors in:

* DMC sample timing
* DMC IRQ timing
* interactions with OAM DMA

**Concrete fix direction**

* Introduce a `Complete` (or `PostRead`) phase and only return to `Idle` after its cycle(s) elapse.
* Or at least record “read completed this cycle” separately from “CPU resumes next cycle.”

Right now you’ve collapsed those into one moment.

### 2) Your GET/PUT “APU phase” is a risky abstraction

You’ve hardwired an alternation model:

* GET: reads
* PUT: writes

Then you gate OAM and DMC reads/writes on it.

This can be correct *if and only if*:

* the phase is truly aligned to hardware’s internal bus-use sequencing,
* and it stays aligned during CPU “idle/no-bus” microcycles and interrupt cycles.

But your current phase is toggled every master cycle in `nes_console.rs`, regardless of whether the CPU would have used the bus.

That’s a classic emulator trap: a **free-running phase** that drifts from reality.

**Concrete fix direction**

* Derive phase from the CPU’s actual bus intent (read/write/none) rather than toggling blindly.
* If you keep phase, make it a property of “this CPU cycle parity / internal timing,” not an independent clock.

### 3) OAM PendingHalt is too simplified

Your rule:

```rust
if self.oam.op == PendingHalt {
  if !cpu_is_writing { oam.op = Halt; }
}
```

This implies “DMA halt can happen on any non-write cycle.” That’s not a real model. On hardware, start alignment depends on the CPU’s timing/parity relationship to DMA.

This might still work in practice if your CPU bus intent model maps correctly—but you haven’t provided evidence.

**Concrete fix direction**

* Base halt success on “CPU is performing a read bus cycle” (or more precise: “CPU bus is available to be stolen”), not “not writing.”
* Add trace-based verification against a reference.

### 4) CPU repeated reads: good idea, but your input is weak

The arbiter uses `cpu_halted_addr: Option<u16>` and does a `bus.read_byte(addr)` on no-bus DMA cycles.

That’s correct *if the halted address is correct*.

But in `nes_console.rs` you only update `last_cpu_read_address` when `result.memory_read` is true. If the CPU’s last bus access was a write or a fetch cycle and your CPU core doesn’t expose the true “repeated read” address, you’re guessing.

**Concrete fix direction**

* The CPU must expose the actual “open bus/repeated read address” it would drive during DMA stalls. This is CPU-core specific; you can’t fudge it with “last read address.”
* Add an explicit API like `cpu.get_dma_stall_read_address()`.

### 5) Your arbitration rule “DMC Read wins over OAM Get” is an assumption

You’ve encoded it as policy. It might be correct for some scenarios, wrong for others.

**Concrete fix direction**

* Treat as a hypothesis and verify with DMC+OAM collision tests.
* Add trace diffing against Mesen for collision-heavy sequences.

---

# Review: `nes_console.rs`

## What’s strong

* You now **query CPU bus intent before stepping**:

  ```rust
  let cpu_bus_intent = self.cpu.borrow().get_pending_bus_operation();
  ```

  This is a big architectural upgrade because arbitration must be decided before CPU mutates state.

* You query APU’s DMC request **before ticking APU** (at least in this version), which avoids the “APU discovers DMA after CPU already used the bus” failure mode.

* You keep a scheduler-local `apu_phase` and pass it to the DMA controller explicitly. That’s a clean interface contract, even if the phase model might be wrong.

## What’s wrong / contradictory / dangerous

### 1) Your PPU stepping comment and behavior disagree (and one is wrong)

You wrote:

```rust
// Advance PPU (exactly 3 dots per CPU cycle for NTSC)
let (new_ppu_cycles, ppu_frame) = self.ppu.borrow_mut().run(self.ppu_counter.current, 1)?;
```

You pass `1`, not `3`.

If `ppu.run(current, cpu_cycles)` internally multiplies by 3, fine—but then your comment is misleading.

If it does **not** multiply by 3, your PPU is running at 1/3 speed. That’s catastrophic.

**You need to settle this.**
Either:

* rename the argument to `cpu_cycles` and have PPU internally convert to dots, or
* pass dots explicitly and stop pretending it’s CPU cycles.

Right now you’re mixing conceptual units in a way that screams “future bug.”

### 2) You still have dead/contradictory `pending_dmc_dma`

You now do both:

* immediate request from `apu.needs_dmc_dma()` before tick
* plus a delayed mechanism `pending_dmc_dma` that can start later

But in this file, I still don’t see where you set `pending_dmc_dma = Some(...)`. If it’s set elsewhere, fine. If not, it’s dead again.

**Hard truth:** this is what “Claude code” looks like when nobody deletes the abandoned branch.

**Fix direction**

* Choose one mechanism: immediate arbitration or scheduled delayed start.
* Delete the other.
* If you need delay, schedule it deterministically relative to bus phase, not via an ad-hoc countdown.

### 3) You’re toggling phase unconditionally, which can drift from CPU reality

You do:

```rust
self.apu_phase = self.apu_phase.toggle();
```

every master cycle.

If your CPU has cycles that don’t correspond 1:1 with the GET/PUT expectation (idle cycles, interrupt sequences, DMA cycles, etc.), your phase can drift.

You partially mitigate by using `cpu_bus_intent` and gating halts, but phase remains independent.

**Fix direction**

* Tie phase progression to a single source of truth:

  * master CPU cycle count parity, OR
  * actual CPU bus microcycle sequencing.
* If DMA is active and CPU is stalled, does phase still toggle on hardware? Maybe yes, maybe no depending on what you mean by phase. You need to define this precisely.

### 4) CPU counter always increments by 1 even during DMA, but CPU might not actually advance

You do:

```rust
self.cpu_counter.current += 1;
self.master_cycles += 1;
```

every cycle regardless of DMA.

That’s fine if `master_cycles` means “master CPU clock cycles elapsed,” not “CPU executed cycles.” But your names are ambiguous and you also use `cpu_counter` as CPU time.

If `cpu_counter` is used anywhere as “CPU progress,” you’re lying during DMA.

**Fix direction**

* Maintain two counters:

  * `master_cycles` = elapsed CPU clocks
  * `cpu_exec_cycles` = cycles where CPU actually advanced internal state
* Or rename `cpu_counter` to `master_cpu_cycles` to reflect reality.

### 5) `cpu_cycle_odd` is now legacy but still mutated—and may mislead other subsystems

You still flip it every cycle:

```rust
let new_parity = !self.cpu_cycle_odd.get();
self.cpu_cycle_odd.set(new_parity);
```

But you also randomize it on power-on based on phase.

If anything else in the system still uses `cpu_cycle_odd` for real timing decisions, this is a hidden dependency bomb.

**Fix direction**

* Make `cpu_cycle_odd` private to the DMA subsystem (or delete it).
* Stop exporting this as a shared cell if it’s now “legacy.”

### 6) DMC request timing: pre-tick query is plausible, but you haven’t proven it

This line:

```rust
self.apu.borrow().needs_dmc_dma()
```

before `apu.run(…, 1)` assumes the APU can know *before* its tick that it needs a fetch now.

That can be correct if `needs_dmc_dma()` checks “will expire this cycle,” but it might need to check “expired on previous tick” or “expires after this tick.”

You need to define whether `needs_dmc_dma()` is edge-triggered or level-triggered and which cycle boundary it refers to.

**Fix direction**

* Document and enforce: “needs_dmc_dma() returns Some(addr) iff the DMC fetch should occur on the current master cycle’s GET window.”
* Add an assertion trace to verify that the request aligns with the DMC timer.

### 7) Randomizing APU phase with wall-clock time is not what you want in deterministic testing

You did:

```rust
SystemTime::now().… % 2
```

This makes runs non-deterministic unless tests override it. You said determinism matters earlier; this undermines it.

**Fix direction**

* Gate phase randomization behind config:

  * deterministic mode: fixed phase
  * realism mode: randomized
* Or use a seedable PRNG with a test-controlled seed.

---

# Cross-file integration issues (where you’ll actually fail)

## A) The arbiter still isn’t fed the right CPU signal

Your DMA controller wants: “can halt on reads, not writes.”
Your console passes: `cpu_is_writing`.

That’s still too weak. You need:

* cpu bus op type (read/write/none)
* cpu address (especially for repeated reads)
* possibly “is opcode fetch” vs “data read” if your CPU models these differently

If the arbiter doesn’t see the true CPU bus behavior, it can’t be correct.

## B) You’re mixing “bus arbitration” with “component ticking” without an explicit bus data model

DMA reads and writes happen through the bus in `DmaController`. CPU reads happen through the bus in `CPU.step_cycle()`.

But there’s no explicit shared “data bus” update at the end of each cycle visible in these APIs. If open-bus behavior matters (it does), you need to ensure:

* every bus read/write updates the shared data bus value deterministically
* and the CPU/PPU/APU read from that bus when appropriate

Right now it’s implied, not enforced.

---

# The next steps that matter (ruthless priority)

1. **Resolve the PPU stepping units.**
   This is non-negotiable. Confirm what `ppu.run(..., n)` expects. If it expects CPU cycles, fine; if not, fix it now.

2. **Make CPU provide a real per-cycle bus intent + stall read address.**
   “Last CPU read address” is a hack. Replace it with a CPU API designed for DMA stalls.

3. **Delete `pending_dmc_dma` unless you can justify it with a reference trace.**
   You’re carrying complexity you’re not using.

4. **Make phase deterministic/configurable.**
   Don’t sabotage CI with wall-clock randomness.

5. **Add a one-line-per-cycle trace and diff it against a reference.**
   Until you do that, you’re still guessing.

---

