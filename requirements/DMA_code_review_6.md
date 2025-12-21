DMA update feedback
---

You must fix a **scheduler correctness bug** in the DMA integration.

### Problem (non-negotiable)

The scheduler currently halts the CPU whenever `dma_controller.is_active()` is true.
This is **incorrect** because `is_active()` includes the `PendingHalt` state.

**Resulting bug:**
The CPU is halted **immediately** after an OAM/DMC DMA request, even though the DMA
model explicitly states that `PendingHalt` must wait for a CPU **read cycle** before
the halt actually succeeds.

This breaks cycle-accurate timing and invalidates the DMA timing model.

---

### Required fix

1. Introduce a new, explicit query on `DmaController`, e.g.:

```rust
pub fn is_cpu_stalled(&self) -> bool
```

2. `is_cpu_stalled()` must return `true` **only** when the CPU is actually stalled by DMA:

* **OAM DMA:**
  `Halt`, `WaitGet`, `Get`, `WaitPut`, `Put`

* **DMC DMA:**
  `Halt`, `Dummy`, `Align`, `Read`

3. `PendingHalt` **must NOT stall the CPU**.

4. Update the scheduler (`NesConsole::step_master_cycle`) to:

* Use `is_cpu_stalled()` (not `is_active()`) to decide whether to call `cpu.step_cycle()`
* Use the same condition to set `CpuCycleResult.halted`

---

### Invariants you must preserve

* DMA state machines (`PendingHalt → Halt → …`) must remain unchanged.
* Bus arbitration rules must remain unchanged.
* Exactly **one bus operation per master cycle** must still be enforced.
* CPU continues executing normally during `PendingHalt` until a read cycle allows the halt to succeed.

---

### Acceptance criteria

* Writing `$4014` on cycle **N** does **not** halt the CPU on cycle N.
* First possible CPU stall caused by OAM DMA occurs on cycle **N+1** (or later, if CPU is writing).
* Existing DMA overlap logic (DMC > OAM priority) still works.

If you change anything outside this scope, explain why. If you do not introduce `is_cpu_stalled()`, the fix is considered incomplete.

---

If Claude implements *exactly* that, your DMA model finally matches the architecture you’ve been describing.
