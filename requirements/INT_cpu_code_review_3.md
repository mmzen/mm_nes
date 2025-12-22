
INTERRUPT FLAG LATENCY CODE REVIEW
---

You’re close. This is *way* better architecturally than the earlier version. But there are **two correctness landmines** and **a few design debts** that will bite you later if you don’t fix them now.

I’m going to focus on what matters: **cycle correctness + interrupt correctness + DMA intent correctness**.

---

## ✅ What’s solid

### 1) `pending_interrupt` is now the right idea

You did the important thing: **interrupt cycle 1 happens only in `FetchOpcode`**, and `Executing` only *schedules* interrupts. That’s the cleanest architecture you can have for cycle-stepped 6502.

### 2) `reset()` now clears `pending_interrupt`

Good. Ghost interrupts are a classic “why the hell is this test failing” time sink.

### 3) `compute_pending_bus_intent()` for vector reads is improved

You’re now reporting actual vector addresses for cycle 6/7 instead of `None`. That’s an upgrade for DMA / tracing.

---

## 🚨 Critical bug #1: InterruptSequence cycle numbering is inconsistent (and currently wrong)

You explicitly say:

* “Cycle 1 is done in FetchOpcode”
* You enter `InterruptSequence` with `cycle: 2`

But then `execute_interrupt_cycle()` still has:

```rust
match cycle {
  1 => read PC
  2 => read PC
  3 => push PCH
  ...
  6 => read vector low
  7 => read vector high
}
```

### What actually happens with your current transitions

* `FetchOpcode`: does dummy read @ PC (this is interrupt “cycle 1”)
* enters `InterruptSequence { cycle: 2 }`
* then `InterruptSequence` calls `execute_interrupt_cycle(..., cycle=2)` which **also** does “read PC”.

So far so good: that’s interrupt “cycle 2”.

But then you increment the cycle state with `cycle + 1` → it becomes 3,4,5,6,7…

That means you will execute **exactly 6 more cycles** inside `InterruptSequence`, giving a total of **7 cycles overall**. Good.

### The problem: your `compute_pending_bus_intent()` assumes cycle 1/2 read PC inside InterruptSequence

But cycle 1 is **never present** in `InterruptSequence` anymore (you start at 2). Yet `compute_pending_bus_intent()` says:

```rust
InterruptSequence { cycle, ... } => match cycle {
  1 | 2 => read PC
  3 | 4 | 5 => write stack
  6 => vector low
  7 => vector high
}
```

This is a mismatch: the state machine never produces `cycle == 1` in InterruptSequence, but the intent logic thinks it might.

✅ Not fatal, but it’s a symptom of a deeper issue: your cycle numbering semantics are muddy.

### Fix recommendation (pick one, don’t half-do it)

**Option A (cleanest):** represent interrupt cycle 1 as `InterruptSequence { cycle: 1 }` and remove the special “do cycle 1 in FetchOpcode” behavior.

* This violates your design goal (“FetchOpcode represents a real cycle”), so probably not what you want.

**Option B (keep your design):** treat `InterruptSequence` cycles as **2..=7 only**, and delete `cycle==1` arms from all intent/execute logic.

* Meaning:

    * `execute_interrupt_cycle()` should accept cycles 2..=7 only
    * `compute_pending_bus_intent()` should accept cycles 2..=7 only

Right now, you’re in an inconsistent middle.

---

## 🚨 Critical bug #2: NMI hijack timing is wrong by one cycle

You say:

> “NMI can hijack an IRQ sequence if it arrives during cycles 1-4”

With your design, “cycle 1” is in FetchOpcode, and your hijack check is:

```rust
if interrupt_type == Irq && cycle <= 4 && self.pending_nmi { ... }
```

But remember: in your InterruptSequence, `cycle` starts at **2**.

That means your hijack window is effectively:

* InterruptSequence cycles 2,3,4
* NOT the FetchOpcode dummy read “cycle 1”
* And you’re not checking hijack in FetchOpcode at all once you’ve committed to IRQ

### Why this matters

On real hardware, NMI can preempt IRQ *very* early in the sequence. Your earlier implementation handled “arrives during cycles 1-4” inside the interrupt handler; now you’re missing part of it.

**Practical effect:** some test ROMs will show wrong vector used (IRQ vector instead of NMI) depending on exact cycle when NMI asserts.

### Fix

If you insist cycle 1 lives in FetchOpcode, then you need hijack logic that covers:

* NMI asserted **after** you decided IRQ but **before** you finish the first interrupt read (or at least before push PCH)

This usually means:

* When entering interrupt handling for IRQ, you should compute `vector_base` as IRQ, but allow it to switch to NMI if pending_nmi goes true **before stack push** begins.

Your current logic only checks during `cycle <= 4` of InterruptSequence (2..4), which is close but not identical.

---

## ⚠️ Design debt: `execute_interrupt_cycle()` ignores `_interrupt_type`

You now pass `vector_base` explicitly, so `_interrupt_type` truly is unused. That’s fine, but be honest:

* remove the parameter or use it for debug assertions.

Right now it’s misleading. And misleading code becomes wrong code later.

---

## ⚠️ Stale latch state: you clear latches in two places, but not consistently

You do clear latches when:

* scheduling pending_interrupt at instruction completion
* committing to service in FetchOpcode

But you **don’t** clear latches when:

* an instruction completes with **no** interrupt
* FetchOpcode runs with `pending_interrupt = None` and no latch set

That’s probably okay because you clear latches before polling in that path, but you should enforce one invariant:

> At the start of FetchOpcode, latched state must reflect *only* what was polled in that cycle or what was deferred.

Right now it’s “mostly true” but not structurally guaranteed.

---

## ✅ The one thing Claude did *right* that you should keep

Adding `vector_base` into the InterruptSequence state is correct. It prevents “vector address depends on interrupt_type + dynamic hijack” mess inside bus intent, and makes tracing sane.

---

## What I would change immediately (minimum viable patch)

1. **Make InterruptSequence cycles explicitly 2..=7**

    * Update `compute_pending_bus_intent()` to drop `cycle == 1` arm
    * Update `execute_interrupt_cycle()` to drop `cycle == 1` arm and re-number (2→PC read, 3→push PCH, …, 7→vector high)
    * Or keep `execute_interrupt_cycle()` as-is but assert you never call it with 1 and never compute intent for 1

2. **Fix hijack window definition**

    * Decide and document precisely:

        * “NMI hijack permitted during FetchOpcode interrupt-cycle-1 read + InterruptSequence cycles 2..4”
    * Then enforce it:

        * either check in FetchOpcode after dummy read (or just before pushing PCH)
        * or expand checks to match hardware constraints

3. **Remove or use `_interrupt_type` in execute_interrupt_cycle**

    * Don’t keep dead parameters.

---

## Bottom line

You’re no longer flailing. The architecture is now coherent.
But **cycle numbering consistency + hijack timing** are the difference between “seems fine” and “passes the nasty ROMs”.


