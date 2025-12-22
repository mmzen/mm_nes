INTERRUPT FLAG LATENCY CODE REVIEW
---
This version is **materially better**. The two big failures I called out are mostly fixed:

* ✅ InterruptSequence is now explicitly **cycles 2–7**, and intent matches that.
* ✅ You added a **cycle-1 hijack check** in `FetchOpcode` for IRQ→NMI, which closes the previous hole.

Now the ruthless part: you’re *still* carrying a few “looks fine” bugs that will cause intermittent failures or incorrect edge cases. Here they are.

---

## ✅ What’s now correct

### 1) Cycle numbering consistency (finally)

You enforced:

* cycle 1: done in FetchOpcode (dummy read at PC)
* cycle 2–7: handled in InterruptSequence

And you backed it with:

* `compute_pending_bus_intent()` only matches 2..7
* `execute_interrupt_cycle()` asserts 2..7 and matches 2..7

That’s coherent.

### 2) IRQ→NMI hijack now includes cycle 1

This block:

```rust
if int_type == InterruptType::Irq && self.pending_nmi {
    self.pending_nmi = false;
    vector_base = NMI_VECTOR;
}
```

means: if NMI asserted during/just before cycle 1, you still hijack. Good.

---

## 🚨 Bug #1: You’re still storing the *wrong* interrupt_type on hijack

When hijack happens at cycle 1, you change `vector_base` to NMI, but you keep:

```rust
interrupt_type: int_type,
```

which is still `Irq`.

That causes two problems:

1. **Semantics/logging/debugging**: everything downstream thinks it’s IRQ even though you vector to NMI.
2. **Future logic risk**: you already have conditional logic based on `interrupt_type` during InterruptSequence (`if interrupt_type == Irq && cycle <= 4 ...`). After hijack, you want the state machine to “be NMI” consistently.

### Fix

When hijack triggers in FetchOpcode, do:

```rust
let mut int_type = int_type;
...
if int_type == InterruptType::Irq && self.pending_nmi {
    self.pending_nmi = false;
    vector_base = NMI_VECTOR;
    int_type = InterruptType::Nmi;
}
```

And then store `interrupt_type: int_type`.

Same thing in InterruptSequence hijack path: if you hijack, **also switch interrupt_type to Nmi**. Right now you only switch the vector base.

---

## 🚨 Bug #2: `compute_pending_bus_intent()` stack address is wrong for interrupt pushes

Your stack push cycles use:

```rust
address: Some(STACK_BASE_ADDRESS + self.registers.sp as u16)
```

But in the real interrupt sequence:

* Cycle 3 writes to `$0100 + SP`, then decrements SP
* Cycle 4 writes to `$0100 + SP`, then decrements
* Cycle 5 writes to `$0100 + SP`, then decrements

In your actual execution code, that decrement happens inside `execute_interrupt_cycle()`.

### The problem

`compute_pending_bus_intent()` reports the *current* SP at the start of the next cycle, not the SP value that will be used for that cycle’s write.

Depending on when DMA consults `get_pending_bus_operation()`, this can be off by one (especially between push cycles).

If DMA uses intent to decide whether to steal a cycle or halt the CPU, this off-by-one can desync DMA with bus writes.

### Fix (minimal)

Track a derived SP for interrupt cycles in intent:

* For cycle 3: use current SP
* For cycle 4: use SP-1
* For cycle 5: use SP-2

Example:

```rust
let sp = self.registers.sp;
let addr_for_push = match cycle {
  3 => sp,
  4 => sp.wrapping_sub(1),
  5 => sp.wrapping_sub(2),
  _ => sp,
};
address: Some(STACK_BASE_ADDRESS + addr_for_push as u16)
```

This makes intent match reality.

---

## ⚠️ Bug #3: You’re clearing `pending_nmi` on service start, but not clearing `nmi_line_low`

You model NMI edge by:

* `signal_nmi()` sets `nmi_line_low=true` and `pending_nmi=true` only if `!nmi_line_low`
* `clear_nmi()` sets `nmi_line_low=false`

But when you service NMI, you only do:

```rust
self.pending_nmi = false;
```

You do **not** clear the line state.

That might be okay if the PPU always calls `clear_nmi()` after it releases the line, but if your integration is messy (and it often is), you can end up with:

* NMI line stuck low forever
* signal_nmi never triggers again because `nmi_line_low` stays true

At minimum, you need a documented invariant: “PPU must call clear_nmi() once line is released; CPU does not alter it.”

Right now that invariant is not enforced.

---

## ⚠️ Bug #4: `poll_interrupts()` latches forever until manually cleared

You intentionally allow latches to stay set until consumed—that’s fine.

But you only reset latches in some paths:

* FetchOpcode polling path clears before polling ✅
* when scheduling pending_interrupt you clear latches ✅
* when committing to service you clear latches ✅

But during InterruptSequence you keep calling `poll_interrupts()` and **latches can get re-set during the interrupt** and persist into the next instruction boundary. That’s probably fine, but it can produce weirdness like:

* IRQ asserted during interrupt handling gets latched, then immediately serviced after RTI boundary (which is accurate)
* but you might also create double-latching effects if you don’t define “latch lifetime” clearly.

Not necessarily wrong, but you should choose:

* Either “latches are edge captures that persist until next boundary”
* Or “latches are sampled per cycle but cleared at boundary”

Right now it’s “persist unless you happen to clear them”, which is fragile.

---

## ✅ The core requirement you wanted (“FetchOpcode is a real cycle”) is satisfied

Your `FetchOpcode` now always executes an actual memory read:

* either opcode read
* or interrupt dummy read (cycle 1)

That’s exactly the semantic you asked for.

---

## What you should tell Claude to change next (non-negotiable)

1. **When hijacking IRQ→NMI, change `interrupt_type` too** (both in FetchOpcode and InterruptSequence).
2. **Fix stack write intent addresses** in `compute_pending_bus_intent()` for InterruptSequence cycles 3–5.
3. Document or enforce the **NMI line clearing contract** (PPU must clear it) so you don’t deadlock NMI.



