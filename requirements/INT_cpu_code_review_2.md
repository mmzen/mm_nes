INTERRUPT FLAG LATENCY CODE REVIEW
---

Here’s the straight code review you asked for. I’m focusing on correctness (cycle accuracy + IRQ latency), then maintainability.

---

## ✅ What you nailed

### 1) **Canonical interrupt cycle 1 in `FetchOpcode`**

This is the right architecture if you want deterministic “interrupt entry latency.” Your `pending_interrupt` + `FetchOpcode does dummy read and enters InterruptSequence at cycle 2` is a clean contract and should fix a lot of edge-case timing.

### 2) **`poll_interrupts()` latches IRQ independent of I**

That change is essential for the AccuracyCoins “Interrupt Flag Latency” class of tests: IRQ can be asserted while I=1, then I clears shortly before boundary — you still need to service immediately once I is clear.

### 3) **Executing completion -> always `FetchOpcode`**

Correct. You removed the biggest source of “double cycle 1” and timing drift.

---

## 🚨 Critical bugs / missing pieces (these will still break tests)

### 1) **`reset()` forgets to clear `pending_interrupt`**

You added the field but didn’t reset it.

If a test ROM triggers an interrupt, then resets CPU, you can carry stale interrupt intent into the next boot and get insane behavior (phantom interrupt entry).

**Fix:**

```rust
self.pending_interrupt = None;
```

in `reset()`.

---

### 2) **`compute_pending_bus_intent()` is now lying during `FetchOpcode`**

Your new `FetchOpcode` can represent either:

* opcode fetch, or
* interrupt cycle 1 (dummy read at PC)

But `compute_pending_bus_intent()` always reports “fetch opcode from PC”.

That sounds harmless because *both read PC*, but it’s not harmless because **DMA halt rules depend on whether CPU is “writing”**, and more importantly, the rest of your system uses this intent as a “contract for next cycle semantics.”

If you ever extend this (and you will), you’ll get bugs because `FetchOpcode` is no longer uniquely “fetch opcode.”

**Fix:** Include intent metadata:

* Add `CpuBusIntent.kind = FetchOpcode | InterruptCycle1` **or**
* Add `Cpu6502::is_fetching_interrupt()` helper and let scheduler ask CPU.

At minimum, update intent to reflect the interrupt case:

```rust
CpuCycleState::FetchOpcode => {
    let is_int = self.pending_interrupt.is_some() || self.latched_nmi || self.latched_irq;
    CpuBusIntent { op: Read, address: Some(self.registers.pc), is_write: false, /* +maybe flag */ }
}
```

Even if address is the same, the *meaning* matters for debugging and later timing rules.

---

### 3) **Latch lifetime semantics are inconsistent**

In `FetchOpcode` you do:

* `pending_interrupt.take()` OR `poll_interrupts() -> check_interrupt_from_latch()`

But then you **clear latches only when committing**.

In `Executing`, you do:

* `poll_interrupts()` every cycle, latches accumulate
* on completion you call `check_interrupt_from_latch()` and store into `pending_interrupt`

What’s missing: after you set `pending_interrupt`, you **should clear latches immediately** (or you’ll potentially re-latch / keep stale). You *do* clear latches when you service in FetchOpcode, but there’s a window where stale latches persist across boundary and can affect later logic or debug output.

**Fix in `Executing` completion:**

```rust
if let Some(int_type) = self.check_interrupt_from_latch() {
    self.pending_interrupt = Some(int_type);
    self.latched_nmi = false;
    self.latched_irq = false;
}
```

---

### 4) **Your NMI edge model is wrong / unused**

You have:

* `nmi_line_low`
* `pending_nmi`
* `prev_nmi_line_low` (but it’s unused)

And `signal_nmi()` sets pending_nmi on `if !nmi_line_low { nmi_line_low = true; pending_nmi = true; }`
That’s not edge detection, that’s “first time we assert NMI until cleared.”

But then `clear_nmi()` sets `nmi_line_low = false` without affecting pending_nmi. So pending can persist across “line high” transitions unless serviced.

Maybe that’s intentional, but then why have `prev_nmi_line_low`?

If AccuracyCoins has NMI behavior tests later, this will bite you.

**Fix:** either remove `prev_nmi_line_low` completely (if you don’t do line polling), or implement actual edge detect:

* store actual line level
* on poll: if prev_high && now_low => pending_nmi = true

Right now it’s half-modelled and will rot.

---

## ⚠️ Interrupt sequence correctness issues

### 5) **Interrupt vector reads in `compute_pending_bus_intent()` are “address: None”**

You already knew this is incomplete, but with DMA bus arbitration, this is not just cosmetic. If DMA uses intent.address, “None” will cause wrong repeated-read behavior during DMA stalls when an interrupt sequence is in progress (worst case: wrong side effects on $2007/$4016 etc).

**Fix:** in InterruptSequence cycle 6/7 you can compute the vector address deterministically from `interrupt_type` and (for IRQ) potential hijack; but hijack depends on pending_nmi mid-sequence.

So either:

* store `vector_base: u16` in `InterruptSequence` state when it starts, and update it if hijacked, OR
* compute it dynamically (but then `compute_pending_bus_intent()` needs access to `interrupt_type` and pending_nmi state)

Right now you can’t answer “what address will be read next cycle?” which undermines your DMA integration story.

---

### 6) **You still have old synchronous interrupt helpers (`interrupt()`, `check_and_setup_interrupt()`, `interrupt_preamble()`)**

These are now basically legacy and dangerous: if anything calls them in the future (or old tests do), you’ll get **double-counting / wrong sequencing**, because now the interrupt path is cycle-stepped.

At minimum: mark them deprecated or `#[cfg(test)]` or rip them out.

Right now your codebase has **two interrupt models** living together. That’s how bugs survive.

---

## ⚠️ AccuracyCoins “Interrupt Flag Latency” still has one more likely failure point: DMC IRQ assertion timing

Even if CPU timing is fixed, your DMC channel can still assert IRQ too late or clear it wrongly.

Your CPU now services IRQ at boundary if:

* `latched_irq == true` and `I == 0`

But `latched_irq` is set by `poll_interrupts()` from `is_asserted_irq()`

So DMC must:

* assert IRQ line *before* that boundary poll
* keep it asserted until $4015 read or explicit clear behavior occurs

If DMC asserts IRQ at “sample end” but clears it too early or asserts one cycle late, CPU won’t see it.

This is not in this CPU file, but don’t expect this patch alone to pass the test unless DMC IRQ level behavior is correct.

---

## Design / maintainability debts (you should fix soon)

### 7) `FetchOpcode` is doing too much work inline

You now have a decent mini-arbiter in `FetchOpcode`. Extract it:

* `fn resolve_fetch_boundary_action(&mut self) -> FetchAction`

    * `FetchAction::Interrupt(InterruptType)`
    * `FetchAction::Opcode(u8, &'static Instruction)`

Makes your semantics explicit and prevents regressions.

### 8) Too many duplicated “if !is_interrupt” branches

`is_interrupt` inside `Executing` cycles is basically dead weight: you never set `is_interrupt: true` for normal instruction execution (interrupts are separate state now). That suggests this field can be removed from `Executing` entirely unless you’re using it for something else later.

---

## Minimal patch list (do these now)

1. Add in `reset()`:

```rust
self.pending_interrupt = None;
```

2. When setting `pending_interrupt` at instruction completion, also clear latches:

```rust
if let Some(int_type) = self.check_interrupt_from_latch() {
    self.pending_interrupt = Some(int_type);
    self.latched_nmi = false;
    self.latched_irq = false;
}
```

3. Make `compute_pending_bus_intent()` reflect interrupt-cycle state better (at least avoid “address None” in InterruptSequence cycles 6/7 and plan vector storage).

---

If you want me to be *brutally* precise: the biggest risk you’re still underestimating is **the mismatch between “cycle-stepped behavior” and “bus intent prediction.”** Your DMA model depends on intent correctness. Right now, interrupt sequences and indirect modes return `address: None` in intent, and that’s the kind of thing that produces “random” failing tests like AccuracyCoins error codes that feel unrelated.

