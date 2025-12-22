INTERRUPT FLAG LATENCY CODE REVIEW
---


Here’s what you must change.

---

## The core rule you must enforce

**Interrupt entry must always consume cycle 1 as a real bus read at PC, and that cycle must not be duplicated.**

So there are only two legal interrupt-entry paths:

1. **At instruction boundary** (your `FetchOpcode` state):

    * perform interrupt **cycle 1** *now* (dummy read at PC)
    * transition to `InterruptSequence { cycle: 2 }`

2. **After an instruction completes** (when leaving `Executing`):

    * the **next cycle is `FetchOpcode`**, and that cycle must do interrupt cycle 1
    * therefore **do not jump directly into `InterruptSequence` from `Executing`**

That’s the entire fix.

---

## Fix #1 (mandatory): Stop entering `InterruptSequence` directly from `Executing`

### Current buggy code (simplified)

When an instruction completes you do:

```rust
if let Some(int_type) = self.check_interrupt_from_latch() {
    self.cycle_state = CpuCycleState::InterruptSequence { cycle: 1, ... };
} else {
    self.cycle_state = CpuCycleState::FetchOpcode;
}
```

This breaks your chosen semantics because it makes the next cycle be “interrupt cycle 1” inside `InterruptSequence`, **instead of being `FetchOpcode` doing the real cycle-1 read**. Even worse, it makes interrupt-entry behavior differ depending on where you enter from.

### Correct behavior under your semantics

Replace it with:

```rust
if let Some(int_type) = self.check_interrupt_from_latch() {
    // Record which interrupt to service at the next FetchOpcode boundary
    self.pending_interrupt = Some(int_type);
    self.cycle_state = CpuCycleState::FetchOpcode;
} else {
    self.cycle_state = CpuCycleState::FetchOpcode;
}
```

Then in `FetchOpcode` you do your existing:

* poll
* check pending_interrupt / latch
* **perform dummy read at PC**
* go to `InterruptSequence { cycle: 2 }`

**Key point:** `InterruptSequence` should *never* start at cycle 1 if `FetchOpcode` owns the real cycle 1.

---

## Fix #2 (mandatory): Add `pending_interrupt: Option<InterruptType>`

You currently clear latches at `FetchOpcode` entry. That means if you “return to FetchOpcode to service an interrupt” you must not lose which interrupt was chosen.

So add to `Cpu6502`:

```rust
pending_interrupt: Option<InterruptType>,
```

Set it when an instruction completes and you decide an interrupt should run.

In `FetchOpcode`, priority order should be:

1. If `pending_interrupt.is_some()` -> service that (don’t re-decide)
2. Else poll/latch and decide normally

Then after starting the interrupt sequence (after dummy read + transition to cycle 2), clear `pending_interrupt`.

---

## Fix #3 (mandatory): Clear latches when you commit to servicing

Once you commit (i.e. you are in `FetchOpcode` and you decided to do interrupt cycle 1), clear:

* `latched_irq`
* `latched_nmi` (or at least the one you are servicing)

Otherwise stale latches can “re-trigger” weirdly.

---

## Fix #4: `compute_pending_bus_intent()` must match this model

If `FetchOpcode` may be “interrupt cycle 1” (dummy read at PC), then `compute_pending_bus_intent()` for `FetchOpcode` must become conditional:

* If `pending_interrupt.is_some()` OR (`latched_*` indicates you will service):
  `CpuBusIntent { Read, address: PC }` ✅ (same as opcode fetch, but semantically “interrupt cycle 1”)
* Else normal opcode fetch intent.

That’s fine — but you must ensure DMA uses the same decision inputs (or it will “see” a read when the CPU is actually about to be stalled for an interrupt).

---

## Why this fixes your DMC IRQ latency failure

The AccuracyCoins “Interrupt Flag Latency” test is basically checking that:

* DMC IRQ becomes active at the right time
* if I is clear at the boundary, the CPU starts the IRQ sequence **immediately at the next fetch boundary**, not one opcode later

Your old path (starting `InterruptSequence` directly from `Executing`) can shift the timing and/or insert the wrong “cycle 1” behavior, which shows up exactly as that test failing.

With the changes above:

* the only place interrupt cycle 1 happens is `FetchOpcode`
* so latency becomes deterministic and matches what your model claims

---

