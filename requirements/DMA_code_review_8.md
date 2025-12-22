DMA feedback update
---

This is now **structurally legit**: one bus op per cycle, OAM writes go through `$2004` via the bus, halted address is captured **when halt succeeds**, DMC phase alignment uses **next_phase**, and you have an explicit `BusWinner`. That’s the right architecture.

Now the sting: there are still **two correctness issues** and **two design risks** that will absolutely bite you if you’re serious about “very accurate.”

---

## ✅ What’s genuinely correct now

* **Halted address capture happens at the right moment**

  ```rust
  if self.oam.op == PendingHalt && !cpu_is_writing {
      self.cpu_halted_addr = pending_read_addr;
      self.oam.op = Halt;
  }
  ```

  This fixes the earlier “freeze too early” bug.

* **OAM PUT is `$2004` and goes through the real bus**

  ```rust
  BusOp::Write(PPU_OAMDATA, self.oam.read_value)
  self.bus.borrow_mut().write_byte(addr, data)?;
  ```

  This is what you need for OAMADDR increment/ignore behavior to be honored by PPU.

* **DMC Read priority over OAM GET** is correct and clean.

* **CPU can continue during `PendingHalt`** — this matches reality (halt attempt failing on writes).

---

## ❌ Correctness issue #1: `cpu_already_halted` logic for DMC PendingHalt is wrong

You do:

```rust
if self.dmc.phase == PendingHalt {
    let cpu_already_halted = self.is_cpu_stalled();
    if !cpu_is_writing || cpu_already_halted { ... }
}
```

But `is_cpu_stalled()` returns true if **DMC is stalled** too:

```rust
let dmc_stalls = matches!(self.dmc.phase, Halt|Dummy|Align|Read);
```

Right now DMC is `PendingHalt`, so it doesn’t contribute, but the intent is wrong: you are using a function that mixes OAM and DMC stall states to decide whether DMC can transition. It works accidentally today; it will break the first time someone tweaks stall state definitions.

**What you actually mean:**

* “CPU is already stalled by OAM (or by an already-successful DMC halt), therefore DMC can proceed even if CPU bus intent is ‘writing’.”

So the correct predicate is:

* “OAM is in a stalling state” OR “CPU already stalled externally”.

**Fix (required):**
Replace `cpu_already_halted = self.is_cpu_stalled()` with an OAM-only check:

```rust
let cpu_already_halted = matches!(self.oam.op,
    OamDmaOp::Halt | OamDmaOp::WaitGet | OamDmaOp::Get | OamDmaOp::WaitPut | OamDmaOp::Put
);
```

Or better, create `is_cpu_stalled_by_oam()`.

This is small, but it’s a correctness landmine.

---

## ❌ Correctness issue #2: `oam_dma_cycles()` helper is wrong / misleading

You have:

```rust
pub fn oam_dma_cycles(write_phase: ApuPhase) -> u16 {
    if write_phase.is_get() { 513 } else { 514 }
}
```

This is not reliably true as written because the true 513 vs 514 depends on **whether the cycle after the halt cycle is GET or PUT**, not “phase when $4014 was written.” Those are often correlated but not equivalent once you model halt-attempt failures on CPU writes.

If the halt is delayed (because CPU was writing), the phase at which the halt succeeds can change relative to the write phase.

**Fix (required):**
Either delete this helper or rename it to what it really is:

* “expected count given an assumed immediate halt and known alignment,” and clearly mark it as test-only.
  For accuracy, compute cycle count dynamically from the state machine instead of pre-calculating.

---

## ⚠️ Design risk #1: You’re still “starting OAM DMA” immediately on seeing `dma_start_page`

In the console loop, you do:

```rust
if let Some(page) = oam_dma_start {
    self.dma_controller.start_oam_dma(page);
}
```

Whether this is correct depends on *when* `dma_start_page` is set by the $4014 write.

If it’s set on the same CPU cycle as the write, your DMA begins one cycle too early. On hardware, OAM DMA begins halting **on the first cycle after the $4014 write**.

If `PpuDma` sets the signal on the write cycle, your scheduler must defer the `start_oam_dma()` until the next cycle.

You need a guaranteed contract here:

* `$4014` write cycle sets a latched request, and scheduler services it on the next CPU cycle.

**Fix (required if not already true):**
Make `PpuDma::write($4014)` set `dma_start_page` and **also** store “arm for next cycle” semantics, e.g.:

* `dma_start_page_next = Some(page)` and swap at end of cycle, or
* store a boolean `oam_dma_armed` that becomes active on next tick.

Right now, the correctness depends on hidden timing in `PpuDma`.

---

## ⚠️ Design risk #2: DMC scheduling remains fully outside DMA controller, but you’re passing `pending_read_addr` into `request_dmc_dma` anyway

Your `request_dmc_dma` signature still has `_conflict_address`, and you’re passing `pending_read_addr` to it, but the controller ignores it.

This is a signal you’re halfway between two models:

* “APU is authoritative and DMA is dumb executor”
* “DMA knows conflict address / repeated reads”

Pick one and cleanly delete the other.

Given your goal (very accurate), the correct approach is:

* Keep the halted/repeat address logic inside DMA controller (already done).
* Remove the unused “conflict address” parameter unless you truly implement the mixed-decode APU/joypad conflict behavior later.

Right now it’s noise that misleads future you.

---

## One more subtlety you should validate with a test (not a guess)

When both DMAs are active and neither needs the bus on this cycle, you do CPU repeat read:

```rust
if self.is_active() && self.cpu_halted_addr.is_some() { CpuRepeat }
```

This is good, but only if:

* the CPU is actually stalled during those phases (`is_cpu_stalled()` true), which you now compute and expose.

Your arbiter currently uses `is_active()` not `is_cpu_stalled()` to decide CPU repeat. That can matter during `PendingHalt`, where DMA is active but CPU is not stalled. In that case, you must NOT steal the bus with CpuRepeat.

You avoided it most of the time because you don’t set `cpu_halted_addr` until halt succeeds, but it’s still the wrong predicate.

**Fix (recommended):**
Change:

```rust
if self.is_active() {
```

to:

```rust
if self.is_cpu_stalled() {
```

That makes it correct by construction.

---

## What I would do next (ruthless priority)

1. **Fix DMC PendingHalt “already halted” predicate to OAM-only.**
2. **Change CpuRepeat condition to `is_cpu_stalled()`** (not `is_active()`).
3. **Delete or quarantine `oam_dma_cycles(write_phase)`** — it’s misleading.
4. **Audit $4014 signal timing** so `start_oam_dma` occurs the cycle *after* the write, not same cycle.

Do those four and you’re in “credible accuracy” territory.


