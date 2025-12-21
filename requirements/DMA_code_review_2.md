DMA update feedback
---

This is **much closer** to something I’d trust. You finally enforce the one rule that matters: **one bus op per CPU cycle**. That single change kills 80% of the subtle “ghost side effects” bugs.

But I’m not going to let you ship this yet, because there are still several correctness holes. Some are “will definitely break accuracy”; some are “will break in edge cases”; and a couple are “you think you implemented X but you didn’t.”

I’ll be precise.

---

## 1) The biggest remaining correctness bug: OAM DMA PUT is not writing to $2004

You currently model OAM PUT as:

```rust
BusOp::Write(self.oam.byte_index, self.oam.read_value)
...
self.ppu.borrow_mut().dma_write(addr as u8, data)?;
```

That is wrong.

On real hardware, OAM DMA performs a CPU write to **$2004** each PUT cycle. The byte index is part of the **source** address; destination is always $2004, and PPU increments OAMADDR internally.

Your current model is writing to an “address” equal to the byte index (0..255), and then passing that as `addr as u8` to `dma_write`. That only makes sense if your `dma_write(oam_index, value)` API is a fake “direct OAM write by index” escape hatch. If that’s the case, you’re not emulating PPU’s OAMADDR behavior correctly (and you’ll break mid-DMA OAMADDR effects and rendering-time ignore behavior).

**Fix (required):**

* Bus op for PUT must be `Write(0x2004, data)`.
* The PPU side must handle “write to $2004” semantics (use current OAMADDR, increment/wrap, ignore if rendering).
* If you insist on keeping `dma_write(index, value)` for convenience, it must internally emulate exactly what $2004 would do, including OAMADDR effects. Otherwise it’s inaccurate.

Right now, it’s not a bus model. It’s a shortcut.

---

## 2) DMC DMA phase advancement is wrong because you advance phases after executing the bus op

You do:

* compute wants
* arbitrate
* execute bus op
* then:

    * `if oam Halt -> WaitGet`
    * `advance_dmc_no_bus_phases()`

The DMC sequence is: **Halt cycle** → **Dummy** → **Align (optional)** → **Read**.
Those are **cycle-aligned states**, meaning the transition to the next phase should occur **at the end of the cycle that represented that phase**.

Your `advance_dmc_no_bus_phases()` is called every cycle unconditionally, and it uses current `apu_phase` to decide Dummy→Align/Read:

```rust
if self.apu_phase.is_put() { self.dmc.phase = Align } else { self.dmc.phase = Read }
```

But note: `apu_phase` refers to the **current cycle**, and the toggling happens outside this module. Depending on when the caller toggles, you may be checking the wrong cycle’s phase.

More importantly: you can enter `DmcDmaPhase::Read` at the end of the cycle, and then in the same cycle’s `oam_wants_bus_op()` you might have already computed something earlier based on the previous phase. This is a classic off-by-one hazard in cycle state machines.

**Fix (required):**
Make the phase machine transitions explicit and tied to the cycle that just occurred, and do not inspect `apu_phase` in a way that depends on when the caller toggles.

A clean rule:

* `step_cycle()` should accept `apu_phase` **as-of this cycle** and should not rely on internal toggling assumptions.
* Transitions should reflect “we just consumed one cycle in phase X.”

Right now, it’s likely to be off by one on align cases.

---

## 3) DMC DMA scheduling is still missing (load vs reload timing)

Your DMC DMA begins at `PendingHalt` immediately upon request:

```rust
pub fn request_dmc_dma(&mut self, address: u16, ...) {
    self.dmc.phase = PendingHalt;
}
```

But actual hardware has **different scheduling rules**:

* reload requests begin halt attempts aligned to a PUT cycle
* load requests begin on the GET of the second APU cycle after enable

That is not present here at all. That means even if your per-cycle sequence is right, you’ll still be wrong on *when the sequence begins*.

If your APU layer guarantees it only calls `request_dmc_dma()` at the correct cycle boundary (and with the correct phase), fine. But then your controller needs to enforce that contract or it will rot.

**Fix (required):**
Add an explicit “scheduled start” state for DMC:

* `ScheduledLoad { target_apu_cycle }`
* `ScheduledReload { wait_for_put }`

or at minimum:

* `PendingStartOnGet`
* `PendingStartOnPut`

and only then move into `PendingHalt`.

Otherwise you’ve shoved correctness out of the DMA controller and into the scheduler/APU in a way that will be very hard to keep correct.

---

## 4) OAM DMA “halt cycle” is not correct unless the console loop blocks CPU on the same cycle

You do:

* if `PendingHalt` and `!cpu_is_writing`, set `op = Halt`
* then `oam_wants_bus_op()` returns None for Halt
* then after bus op, `if self.oam.op == Halt { self.oam.op = WaitGet }`

This models “the halt cycle consumes 1 cycle with no DMA bus op.” Good.

But the hardware detail is: **the halt cycle is the cycle on which RDY first successfully halts the CPU**. If your outer loop starts DMA “one cycle late” relative to the $4014 write, or if the CPU is still allowed to perform its own bus op during that halt cycle, you’re wrong.

So: this depends on the **caller** using `is_active()` to stall CPU on the same cycle `PendingHalt → Halt` occurs.

Your current design can be correct, but only if the console step is strictly:

1. call dma.step_cycle(cpu_is_writing)
2. if dma active, CPU does not step

If your scheduler checks `is_active()` *before* calling `step_cycle()` you might accidentally allow CPU to run on the halt cycle.

**Action:** enforce that the DMA controller returns `cpu_halted` based on **next-cycle**/current-cycle semantics, not `self.is_active()` after the fact. Right now you set:

```rust
result.cpu_halted = self.is_active();
```

after mutating states, which is better than checking it before, but your outer loop must follow it.

---

## 5) OAM DMA completion counting is inconsistent

You track both `byte_index` and `bytes_transferred`. You increment both:

```rust
self.oam.bytes_transferred += 1;
self.oam.byte_index += 1;
if self.oam.bytes_transferred >= 256 { ... }
```

`bytes_transferred` is redundant. More importantly: it creates risk of divergence if any future change increments one but not the other (and Claude will eventually do that).

**Fix:** remove `bytes_transferred`. Use `byte_index == 256` as completion.

---

## 6) DMC alignment rule is implemented incorrectly

You do Dummy → Align when `apu_phase.is_put()`.

But the alignment rule is: after dummy, if the *next needed read* would fall on a PUT, insert an align to land on GET.

Whether that is true depends on the phase **of the cycle following dummy**, not the current phase at the end of dummy. This is a subtle “what does `apu_phase` represent” issue.

This ties back to issue #2: your controller doesn’t own the phase toggle, so this is likely wrong unless you carefully define the contract with the caller.

**Fix:** pass both `current_phase` and `next_phase` into step_cycle, or define `apu_phase` strictly as “phase of this cycle” and compute next as toggle. Then make alignment decision based on `next_phase`.

---

## 7) Your DMC arbitration check `dmc_wants == BusOp::Read(addr)` is fragile

You do:

```rust
if dmc_wants == BusOp::Read(addr) && self.dmc.phase == Read { ... }
```

This can fail if:

* arbiter returns CPU repeated read of same addr
* or DMC wants read but loses due to something else (shouldn’t happen by priority but still)

Better: track the bus winner explicitly in arbiter (e.g. `Winner::DmcRead|OamGet|OamPut|CpuRepeat|None`). Don’t infer it by comparing enum values.

---

## The big picture verdict

### What you fixed (huge)

* ✅ one bus op per cycle
* ✅ DMC Read priority over OAM GET
* ✅ OAM GET/PUT gating exists

### What is still wrong (must fix)

* ❌ OAM PUT destination is wrong (must be $2004 semantics)
* ❌ DMC scheduling (load vs reload start timing) is missing
* ❌ DMC dummy/align timing likely off-by-one unless phase contract is nailed
* ❌ alignment decision uses the wrong phase reference (current vs next)
* ❌ winner inference is brittle

If you fix those, then you’re actually on track to “very accurate.”

---

## What I’d do next (ruthless prioritization)

1. **Fix OAM PUT to $2004** and remove direct-index `dma_write` unless it perfectly emulates $2004.
2. **Make phase contract explicit**: `step_cycle(cpu_is_writing, apu_phase_this_cycle)` and compute next internally; alignment uses next.
3. Add **DMC start scheduling states**; do not rely on APU calling at the perfect cycle.
4. Replace bus-op inference with an explicit `winner` enum from `arbitrate`.

Do those four, and we can start trusting tests.

If you want me to review for true accuracy, paste:

* how $4014 triggers `start_oam_dma` relative to the write cycle
* how `cpu_is_writing` is computed (is it real bus op direction for that cycle?)
* how/when `apu_phase` is toggled in the outer loop
* your PPU `$2004` implementation and whether writes are ignored during rendering
