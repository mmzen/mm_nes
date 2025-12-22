DMA feedback update
---

---

## What’s genuinely strong now

### 1) One bus op per cycle is enforced at the scheduler level

You compute `dma_used_bus` from `dma_result.bus_op` and only step the CPU if:

* CPU is not stalled, and
* DMA did not use the bus

That’s the correct arbiter pattern. It prevents double side-effects and makes timing predictable.

### 2) Halted read address is captured at the only correct moment

Inside DMA:

* OAM: captures `pending_read_addr` when `PendingHalt → Halt`
* DMC: captures if not already captured by OAM when `PendingHalt → Halt`

That’s the correct RDY semantics, and it’s the main reason your conflict/side-effect tests will start matching hardware.

### 3) OAM PUT is a real `$2004` write through the bus

That’s non-negotiable for accurate OAMADDR behavior and “writes ignored during rendering” behavior (as long as the PPU device implements that).

### 4) You fixed the subtle “CpuRepeat during PendingHalt” bug

Arbiter uses `is_cpu_stalled()` not `is_active()`. That’s the difference between “CPU keeps running until halt succeeds” and “DMA steals the bus too early.”

---

## Accuracy issues still present

### A) DMC DMA “Read state waiting” can create an extra cycle you don’t want

Right now DMC’s `Read` phase returns `BusOp::None` if `current_phase` isn’t GET:

```rust
DmcDmaPhase::Read => if current_phase.is_get() { Read(addr) } else { None }
```

But your own Dummy→Align decision is supposed to guarantee the Read cycle lands on GET. If it doesn’t, you silently “wait” in Read with no bus op, which can add an extra cycle beyond 3/4.

Your debug_assert catches only the case where `winner == DmcRead` and phase isn’t GET. It does **not** catch the “Read state on PUT” condition.

**Fix (recommended):**
Add a debug assert at the top of `step_cycle`:

* If `self.dmc.phase == DmcDmaPhase::Read`, then `current_phase` must be GET.
  If it’s PUT, you’ve mis-sequenced Dummy/Align or phase toggling.

That makes the model strict instead of “self-healing”.

---

### B) OAM alignment is implicit, not explicit

You’ve documented “alignment cycle 0–1” and rely on `WaitGet` to serve as alignment.

This can be correct *if* the scheduler’s phase definition is stable and halt occurs on the correct cycle. But it’s fragile because:

* there is no explicit “alignment cycle” state,
* the number of “no-bus” cycles before first GET read becomes an emergent property of phase + halt timing.

That’s fine, but you need a test that pins it down:

* starting on a phase that yields 513, and one that yields 514,
* and ensure you’re not accidentally counting an extra idle due to `PendingHalt` delay.

**Fix (test-level):**
Write a bus-trace test around `$4014` that asserts:

* 1 halt cycle occurs (no DMA bus op)
* then either 0 or 1 extra “no-bus alignment” cycle
* then 256 GET reads and 256 PUT writes alternating with the phase

If that test passes, implicit alignment is acceptable. If it’s hard to express, add an explicit Align state.

---

### C) DMC scheduling is still “APU owns it” — that’s okay, but you must enforce the contract

`needs_dmc_dma()` is called every cycle and you push requests. DMA ignores duplicates.

That’s good. But your correctness now depends on **APU generating the request on the right cycle**, especially:

* load DMA start timing
* reload DMA start timing

If you don’t already have tests that verify `needs_dmc_dma()` fires on the correct cycle boundaries relative to GET/PUT, you’re not accurate.

This is a core truth: you can’t “review” your way to accuracy here; you need a test that fails if it’s off-by-one.

---

## Scheduler-level risk

### D) You’re stepping DMA before CPU when DMA is active

You compute `cpu_bus_intent` first, then step DMA (if active), then maybe step CPU.

This is fine if your `get_pending_bus_operation()` truly reflects the bus op for the current cycle even if you haven’t executed it yet. Since you said the hang is fixed, it’s probably now consistent.

But lock it down with a cheap invariant:

* If DMA does **not** use the bus and CPU is **not** stalled, CPU must step exactly once.
* If DMA uses the bus OR CPU is stalled, CPU must not step.

You already implement it. Add a debug assertion around it so future refactors don’t reintroduce double-stepping or starvation.

---

## Improvements that are actually worth doing

### 1) Remove dead “phase management” methods from DmaController

You have `#[cfg(test)] set_apu_phase/get_apu_phase/toggle_apu_phase` that are no-ops.

That’s confusing and will lead to tests that look like they’re setting phase but aren’t. If tests need phase control, they should pass it through `step_cycle()` explicitly (as production does).

**Fix:** delete these no-op methods. If you need test helpers, make them real and local to tests/harness.

### 2) Return value from `request_dmc_dma` is good — actually use it

You now return `bool`. Use it in the console for debugging:

* if APU requests every cycle while DMA active, you’ll see it’s being rejected constantly.
  This is useful for catching APU bugs early.

---

## The one missing piece I still can’t verify from this code

Whether `$4014` request is latched and starts “next cycle after write”.

You read `dma_start_page` and start DMA immediately in the same master cycle. That’s only correct if `dma_start_page` is set by the **previous** master cycle’s bus write to `$4014`, not by “this cycle”.

If your PpuDma bus device sets it during the write, then the scheduler seeing it at the start of the same cycle would be wrong. If it sets it and you only observe it next cycle, it’s correct. I can’t confirm without PpuDma.

---

## Short verdict

This is now “real” DMA infrastructure, not cosplay. The remaining work isn’t big design; it’s:

* stricter assertions (so timing can’t drift silently),
* explicit tests for 513/514 and DMC 3/4 timing,
* verifying `$4014` trigger timing is one-cycle delayed.

