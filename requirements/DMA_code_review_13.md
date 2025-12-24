DMA feedback update
---


### Code review: phase derivation + DMA/CPU arbitration updates

#### Summary

These changes move the scheduler from “mutable phase toggling” to a **derived phase invariant** and tighten the DMA arbitration contract. That’s a real improvement: you eliminated an entire class of drift bugs and made it easier to reason about *exactly one bus op per master cycle*.

That said, you still have a few correctness landmines and one big unresolved modeling gap (DMC completion timing). Don’t confuse “clean architecture” with “verified hardware timing.”

---

## What’s good

### ✅ Phase is now derived, not mutated

* Replacing `apu_phase.toggle()` with `ApuPhase::from_cycle(master_cycles + phase_offset)` is the right move.
* This prevents silent phase drift during DMA stalls, atypical CPU sequences, or any future scheduler refactors.

**Net effect:** phase parity becomes a deterministic function of master time, not scheduler behavior. That’s exactly what you want for cycle-accurate arbitration.

### ✅ Phase randomization is now testable

* `phase_offset` + `power_on_deterministic(seed)` gives you a deterministic handle for CI.
* This is strictly better than wall-clock randomness when you want reproducible traces.

### ✅ Scheduler arbitration is still enforcing the correct invariant

* You still gate CPU stepping on:

    * `dma_used_bus` OR `is_cpu_stalled()`
* The `debug_assert_eq!(cpu_result.halted, expected)` is a good guardrail against double-stepping and starvation bugs.

---

## What’s risky / incorrect

### 🔥 Bug: `wrapping_sub(1)` makes trigger-phase logic wrong at cycle 0

You compute:

```rust
ApuPhase::from_cycle(self.master_cycles.wrapping_sub(1) + self.phase_offset);
```

If `master_cycles == 0`, `wrapping_sub(1)` becomes `u64::MAX`, which flips parity and makes the assertion meaningless at startup.

**Fix:**

* Use `saturating_sub(1)` for debug-only reasoning, or just remove this “trigger phase” assert entirely.
* If you actually need the trigger phase for correctness later, store it explicitly at the $4014 write boundary.

### ⚠️ OAM DMA “natural alignment” is still an assumption

Your comment claims 513/514 falls out naturally via phase gating. That’s only true if:

* `$4014` write timing is captured at the correct bus cycle boundary, and
* your “phase” parity matches the actual DMA alignment rule

But OAM halt still hinges on:

```rust
if !cpu_is_writing { PendingHalt -> Halt }
```

That’s a coarse proxy. Hardware alignment depends on the **actual bus cycle type**, not “not a write.” If your CPU has internal/no-bus cycles or your intent model is incomplete, you can shift the start by one cycle.

**Recommendation:**

* Evolve CPU intent from “is_write” to an explicit `{Read, Write, None}` and allow halt only on real **read bus cycles**.
* If `None` exists, define whether DMA can halt there (usually no).

### ⚠️ The “pending read address fallback” is still hiding CPU intent bugs

You still do:

```rust
cpu_bus_intent.address.or(self.last_cpu_bus_address)
```

That’s fine as a defensive fallback, but it masks the real issue:

* if `CpuBusIntent.address` is `None` during cycles where the real CPU drives an address, your arbitration inputs are wrong.

**Recommendation:**

* For cycle-accurate mode, make intent return an address on every non-idle bus cycle.
* If a cycle truly has no bus op, return `op=None` explicitly and stop treating “None address” as “maybe.”

### ⚠️ DMC DMA completion timing remains unproven

In the DMA controller you still do:

```rust
self.dmc.phase = DmcDmaPhase::Idle;
```

immediately after the read. That might be correct, but it’s exactly the kind of “off-by-one release” that fails DMC timing ROMs.

**Recommendation:**

* Add a `PostRead`/`Complete` phase (even if it’s 0 cycles initially) so you have an adjustment knob when traces/tests show a mismatch.

---

## Naming/semantics issues

### 🧠 “ApuPhase” is misleading

What you now compute is **cycle parity with an offset**, not necessarily “APU phase.” If you keep the name, future you will attribute failures to the wrong subsystem.

**Suggestion:**

* Rename to `BusPhase` or `DmaPhase`
* Rename `phase_offset` to `cycle_parity_offset`

This is not cosmetic—it reduces debugging misdirection.

---

## Suggested next steps (high ROI)

1. Replace the trigger-phase debug logic:

    * remove `wrapping_sub(1)` or use `saturating_sub(1)` for asserts.
2. Upgrade CPU bus intent:

    * from `(is_write, address: Option<u16>)` to explicit `op: {Read, Write, None}` and address required for Read/Write.
3. Add a hard arbiter invariant:

    * If DMA returns `BusOp::Read/Write`, assert CPU performed **no** bus op that cycle (not just “halted=true”).
4. Add a DMC post-read phase placeholder:

    * gives you a clean knob when DMC tests inevitably disagree.

---

## Verdict

This is a meaningful architectural improvement: phase derivation was the right move and it makes the system more deterministic and auditable. But you still haven’t *proven* correctness—your halt eligibility, intent completeness, and DMC release timing are still the most likely sources of test ROM failures.
