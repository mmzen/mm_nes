INTERRUPT FLAG LATENCY CODE REVIEW
---

You asked for a full review “based on all inputs”. Here it is, no sugarcoating:

This DMA controller rewrite **looks clean on paper** but it has **two correctness landmines** that will absolutely break cycle-accuracy under contention.

---

# 1) Your OAM GET/PUT gating is **not actually phase-gated**

You *say* “reads on GET, writes on PUT”. But your state machine violates its own rule.

### The bug

In `oam_wants_bus_op()`:

```rust
OamDmaOp::Get => BusOp::Read(addr)        // NO PHASE CHECK
OamDmaOp::Put => BusOp::Write($2004, v)   // NO PHASE CHECK
```

So if OAM ever remains in `Get` on a PUT cycle (or `Put` on a GET cycle), it will request an illegal bus op for that phase.

You *hope* this never happens because you only transition into `Get` on GET and into `Put` on PUT — but you already introduced arbitration that can steal cycles. That creates exactly the scenario where OAM can be left in `Get`/`Put` on the “wrong” phase.

Your debug asserts will catch some of it, but that just means you’ll crash in debug and silently misbehave in release.

### Fix (pick one approach)

**Best approach:** delete `Get` and `Put` states entirely. They add no value.

Use only:

* `WaitGet` (on GET → emit read and immediately move to `WaitPut`)
* `WaitPut` (on PUT → emit write and immediately move to `WaitGet` or `Idle`)

That way **phase correctness is structurally guaranteed**.

If you insist on keeping `Get`/`Put`, then:

* gate them by phase (return `None` if wrong), and
* handle “stolen cycle while in Put” the same way you handled “stolen cycle while in Get”.

Right now you only revert `Get` when DMC steals. You do *not* revert `Put` under any arbitration or starvation scenario.

---

# 2) You mutate state *before* arbitration, then “patch it up” after. That’s fragile.

In `oam_wants_bus_op()` you do:

```rust
self.oam.op = OamDmaOp::Get;
return BusOp::Read(addr);
```

before arbitration decides if that read actually happens.

Then in `arbitrate()` you try to undo it only for one case:

```rust
if matches!(self.oam.op, OamDmaOp::Get) { self.oam.op = WaitGet; }
```

That’s a partial band-aid.

### Why this is a design flaw

**A state machine should only advance when its action actually occurs.**
Right now it advances when it *wants* to act, then you try to rewind depending on who wins. That’s exactly how you get off-by-one stalls and rare timing ghosts.

### Fix

Make `*_wants_bus_op()` pure (no mutation). It should return an intent.

Then after arbitration, the winner commits state transitions.

This will also make your “exactly one bus op” invariant easier to reason about.

---

# 3) DMC sequencing: your phase alignment logic is mostly correct, but your “next_phase” contract is dangerous

You compute:

```rust
let next_phase = current_phase.toggle();
...
self.advance_dmc_no_bus_phases(next_phase);
```

This assumes:

* the scheduler’s phase toggles exactly once per master cycle
* the passed `current_phase` is the “true” phase for *this* bus op

Your console currently does toggle once/cycle, **but** the moment you refactor the console’s phase update order (or introduce PAL ratio quirks), this breaks.

### Fix

Don’t pass `next_phase`. Pass a single “phase + parity” primitive from the scheduler that is the only truth. Or keep it as-is but add a hard invariant comment in the console: “phase toggles once per master cycle after bus op”.

---

# 4) Your `cpu_halted_addr` contract is still not good enough (and you already know it)

You capture:

```rust
self.cpu_halted_addr = pending_read_addr;
```

when halt succeeds.

But your console’s `pending_read_addr` is still fundamentally limited: it’s derived from intent and falls back to `last_cpu_read_address`.

That is **not the same thing** as “the last CPU bus address”.

If the CPU is about to do a write when DMA starts, the correct “open bus / repeated read” behavior is not reliably modeled by “last read address”.

### Fix (high priority)

Track `last_cpu_bus_address` (read or write address) in the console, update it on every CPU bus op, and use that for repeated reads when the CPU is stalled and DMA doesn’t own the bus.

Your current “repeat read can cause $2007 side effects” comment is right — but you’re still feeding it the wrong address source.

---

# 5) OAM DMA length: you modeled alignment implicitly, but your own helper is misleading

Your helper:

```rust
pub fn oam_dma_cycles(write_phase: ApuPhase) -> u16 {
    if write_phase.is_get() { 513 } else { 514 }
}
```

This is not reliably true under your model, because **halt does not necessarily succeed immediately** (PendingHalt can delay). You even warn about that — good — but then the helper still encourages incorrect thinking.

If you keep it, I’d rename it to something brutally explicit like:

`oam_dma_cycles_if_halt_succeeds_immediately(write_phase)`

---

# 6) Arbitration priority is plausible, but “DMC Read wins over OAM Get” implementation is incomplete

You implemented:

* DMC read steals and forces OAM `Get` → `WaitGet`

But there are other “steal” cases:

* OAM `Put` can also get delayed (not by DMC read directly since DMC reads only, but by your own scheduler or future expansions).
* Your current model has no robust “stay in WaitPut until PUT happens” guarantee, because you allow being in `Put` state without phase gating.

This circles back to issues #1 and #2. Fix those and arbitration becomes sane.

---

# Bottom line verdict

### CPU changes

Your CPU cycle state machine direction is good (FetchOpcode owns interrupt cycle 1, latching makes sense). Biggest remaining correctness risk is still **bus intent vs real bus address tracking**.

### Console scheduler

The structure is correct. The weak spot is still the repeated-read address sourcing.

### DMA controller (this code)

This rewrite is **not correct yet**. It *claims* to be phase-gated and single-bus-op, but it violates its own phase model via `Get`/`Put` states and state mutation-before-arbitration.

---

# What to do next (non-negotiable)

1. **Rewrite OAM DMA state machine to eliminate `Get` and `Put` states.**
   Only `WaitGet` and `WaitPut`, and only those states can emit bus ops.

2. **Make wants/intents pure. Commit transitions only after bus winner executes.**
   No pre-mutation, no rollback hacks.

3. **Fix repeated-read address source in the console.**
   Track `last_cpu_bus_address` and use it when stalled.

4. Add debug assertions that enforce:

* “OAM read only on GET”, “OAM write only on PUT” at the point the bus op is returned
* “If cpu_halted and BusWinner::None, then CpuRepeat must still happen” (or explicitly justify why not)

