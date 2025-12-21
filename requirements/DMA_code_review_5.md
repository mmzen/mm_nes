DMA update feedback
---

This version is **meaningfully better**. You fixed the most important conceptual bug: you now capture the **pending** read address (the one being halted), not “some last read from history.” That’s the difference between “DMA looks right” and “DMA causes the exact weird side effects real hardware does.”

But you still have a couple of accuracy traps. If you don’t address them now, you’ll end up with tests that pass for the wrong reasons.

---

## ✅ What you fixed correctly

### 1) Capturing the halted read address from *pending* bus intent

```rust
let pending_read_addr = if !cpu_is_writing {
    cpu_bus_intent.address
} else {
    self.last_cpu_read_address
};
```

This is the correct intent: the RDY-stalled CPU repeats the read cycle it was halted on, so you must use the pending read address.

### 2) You explicitly declared the DMC scheduling contract

You wrote it clearly: **APU is authoritative**, DMA is executor. That’s a valid architecture *if you test it hard*.

### 3) No legacy parity noise anymore

You removed `cpu_cycle_odd`. Good. That closes a future “two sources of truth” regression.

---

## ❌ The remaining accuracy traps

### Trap 1 — Your fallback on CPU write cycles is still suspect

When `cpu_is_writing`, you set `pending_read_addr = last_cpu_read_address`.

But remember: **DMA halt cannot succeed on a write cycle**. So if the CPU is writing, you’re not actually halting *this cycle*. You’re halting later, on the first cycle where the CPU is reading.

So the correct “halted address” is **the read cycle where the halt finally succeeds**, not the last read that happened sometime earlier.

Right now you set halted addr at DMA start based on a write-cycle fallback. If `PendingHalt` delays for a few cycles, your frozen halted address may be wrong.

**Fix (required, small):**

* Do *not* freeze the halted address at DMA start.
* Instead, update it **at the moment the DMA controller actually enters Halt** (i.e., when `PendingHalt → Halt` transition occurs), using the CPU bus intent for that cycle.

That requires one of:

* pass the current `cpu_bus_intent.address` into `dma_controller.step_cycle(...)` each cycle, or
* add a `dma_controller.try_set_halted_addr(pending_read_addr)` call each cycle while DMA is pending, and the DMA controller captures it only when halt succeeds.

Right now you’re doing a one-shot set which can be wrong if the halt is delayed.

---

### Trap 2 — Your scheduler still asks `needs_dmc_dma()` before ticking APU

That’s fine **only if** `needs_dmc_dma()` is defined as “a DMA must begin on this CPU cycle based on APU state at the *start* of this cycle.” That’s what you claim.

But it’s very easy for that to rot: someone later changes APU timing and now `needs_dmc_dma()` wants to be evaluated after the APU tick, and suddenly you’re off by one.

**Fix (required):**
Document (in code, not just comments) the exact contract:

* `needs_dmc_dma()` must be pure query.
* It must not advance APU state.
* It must be computed from APU state as of the beginning of the current CPU cycle.
* Its returned address is for the **DMC Read bus op** that will occur after halt/dummy/align phases controlled by DMA controller.

And then enforce it by tests that check start-cycle alignment.

---

### Trap 3 — You’re still gating DMC requests on `!is_dmc_dma_active()`

```rust
let dmc_dma_request = if !self.dma_controller.is_dmc_dma_active() {
    self.apu.borrow().needs_dmc_dma()
} else {
    None
};
```

That may suppress legitimate back-to-back DMC fetch scheduling in edge cases (especially at the boundary where a DMC read completes and a new request is immediately needed). Hardware can request again as soon as the buffer is empty and timer condition triggers.

If your APU never requests again until after it receives `provide_dmc_sample()`, you’re fine — but that’s another implicit contract.

**Fix (recommended):**
Let APU be authoritative and let the DMA controller reject duplicate requests itself if it’s in the middle of a fetch. Don’t suppress at the console layer unless you’re 100% sure.

---

## 🔧 What to change next (minimal surgery, maximum correctness)

### 1) Pass pending read address into DMA step each cycle

Change:

```rust
let dma_result = self.dma_controller.step_cycle(cpu_is_writing, self.apu_phase)
```

to include the pending read address:

```rust
let dma_result = self.dma_controller.step_cycle(cpu_is_writing, pending_read_addr, self.apu_phase)
```

Then inside `DmaController`, when `PendingHalt → Halt` occurs, it captures that address as the frozen RDY repeated read address.

This eliminates the biggest remaining correctness hole.

### 2) Stop setting halted address at DMA start

These lines:

```rust
self.dma_controller.set_halted_read_address(pending_read_addr);
```

should go away (or become a non-freezing “candidate” update). The freezing must happen only when halt succeeds.

### 3) Add tests that force halt delay

You need a test where:

* OAM DMA is triggered while CPU is on a write cycle
* halt is delayed
* the repeated reads occur from the correct later read address (not some old value)

If you don’t test this, you’ll regress it.

---

## Bottom line

You’re no longer “obviously wrong.” You’re now in the territory where only *real* edge tests and precise contracts separate “accurate” from “close enough.”

Right now, the most important remaining flaw is **freezing the halted read address too early**. Fix that, and this scheduler becomes solid.

