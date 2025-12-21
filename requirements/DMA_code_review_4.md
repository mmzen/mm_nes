DMA update feedback
---

You’re getting dangerously close to “actually accurate,” but there are still a few sharp edges that will quietly ruin you if you don’t lock them down now.

I’ll split this into: ✅ what’s now correct, ❌ what’s still wrong/fragile, and 🔧 what to change next with minimal churn.

---

## ✅ What’s now structurally right

### 1) Explicit phase contract

Passing `apu_phase` into the DMA controller per cycle is exactly what you needed. It makes the phase meaning explicit and prevents off-by-one “who toggles when?” bugs.

### 2) CPU bus intent queried before arbitration

```rust
let cpu_bus_intent = self.cpu.borrow().get_pending_bus_operation();
let cpu_is_writing = cpu_bus_intent.is_write;
```

This is the only viable basis for “halt can succeed this cycle.”

### 3) Deterministic power-on phase for tests

`power_on_deterministic()` is the right move. That eliminates flaky tests caused by random initial phase.

### 4) No cycle batching

You advance exactly one cycle for CPU/APU/PPU per `step_master_cycle()`. Good.

---

## ❌ What is still wrong or too weak

### Problem 1 — You never update the DMA controller’s “CPU halted read address” during DMA

When OAM DMA starts you set:

```rust
self.dma_controller.set_halted_read_address(self.last_cpu_read_address);
```

and for DMC DMA request too. But once DMA is active, you don’t update it.

That’s okay only if **the halted read address is frozen** at halt time. On real RDY, the CPU repeats the same read cycle indefinitely, so freezing is correct — **but** you’re freezing the value of `last_cpu_read_address`, which is derived from *past completed cycles*, not “the exact read the CPU is being halted on.”

If the CPU is about to perform a read this cycle, that read address can be different from `last_cpu_read_address`.

**Fix:** when DMA transitions into “halted” state (first successful halt), set the halted address to the CPU’s **pending read address for that cycle**, not the last historical read.

Right now you’re likely repeating the wrong address in the critical “DMC conflict / PPUDATA increment” cases.

Minimum change:

* Use `cpu_bus_intent.address` (if it’s a read) as the halted address when DMA first becomes active / when PendingHalt succeeds.

---

### Problem 2 — Your DMC request call is still missing “load vs reload scheduling”

You still call:

```rust
self.apu.borrow().needs_dmc_dma()
...
self.dma_controller.request_dmc_dma(dmc_address, cpu_is_writing, self.last_cpu_read_address);
```

This assumes the APU is already handling the exact scheduling semantics (load vs reload start cycle). If that’s true, fine, but then your DMA controller is not authoritative and will drift over time as code changes. If it’s not true, your DMC timing is still wrong.

**Hard truth:** you need one place where DMC scheduling lives, and it must be testable. If you keep it in APU, you must prove it with tests and treat the DMA controller as a dumb executor. If you keep it in DMA controller, the APU must only raise an event (“buffer empty and bytes remain”).

Right now it’s ambiguous.

---

### Problem 3 — `last_cpu_read_address` update is still post-step and misses pending reads

You update it after CPU executes:

```rust
let result = self.cpu.borrow_mut().step_cycle()?;
if result.memory_read {
    self.last_cpu_read_address = result.address;
}
```

This is fine for history, but it does nothing for the *current* cycle’s halt behavior. For accurate repeated reads, you need the address of the read cycle that got halted, which is usually the **pending bus op**, not “last completed read”.

So you need a separate field, something like:

* `pending_cpu_read_addr_this_cycle` (from `get_pending_bus_operation()`)

and when DMA is active and wants a repeated read, it should use the frozen halted address captured at the halt moment.

---

### Problem 4 — `cpu_cycle_odd` is still toggled every cycle, which is a footgun

You’ve labeled it legacy, but you still mutate it:

```rust
self.cpu_cycle_odd.set(!self.cpu_cycle_odd.get());
```

That makes it “alive.” Somewhere else will eventually read it and “just use it quickly” and you’ll have two truths again.

If it’s truly legacy-only, lock it down:

* Only update it behind `#[cfg(test)]` or `#[cfg(feature="legacy")]`, or
* Make it private and inaccessible except legacy modules, or
* Delete it.

Leaving it running is how you regress.

---

### Problem 5 — Your DMA controller may be doing the bus ops, but NesConsole doesn’t reflect open bus / side effects explicitly

You convert the DMA bus op to `CpuCycleResult` flags. That’s fine. But accuracy depends on whether your `Bus::read_byte` / `BusDevice` implementation actually triggers register side effects, and whether the data bus open-bus latch is updated properly.

I can’t judge that from here, but you must ensure:

* repeated reads during DMA no-bus cycles are real reads through the bus
* those reads tick controller shift regs and PPU increments etc.

If your bus read path has “pure read” variants or caching, DMA must not bypass side effects.

---

## 🔧 Minimal, high-impact changes you should make next

### 1) Capture halted address correctly (critical)

Add a new local value in step 1:

* if `cpu_bus_intent.is_read`, store `pending_read_addr = Some(cpu_bus_intent.address)` else None.

Then:

* when OAM DMA starts, set halted addr to `pending_read_addr` (not `last_cpu_read_address`)
* when DMC DMA request is issued, same
* and **when PendingHalt succeeds**, update halted addr to the read that actually got halted (again: pending bus intent for that cycle)

This aligns RDY semantics.

### 2) Make DMC scheduling ownership explicit

Pick one and enforce it in code and tests:

* Option A: APU schedules *exact* start cycle and calls `request_dmc_dma()` only when it should begin `PendingHalt`.
* Option B: APU only emits “needs fetch” and DMA controller schedules start based on APU cycle counters and GET/PUT.

Right now you have neither cleanly, and “event-driven” language is masking that.

### 3) Kill `cpu_cycle_odd` or quarantine it

If you want a “single execution path,” you can’t keep a second timing primitive alive, even “just for compatibility.”

### 4) Ensure DMA bus writes go to $2004 semantics

This isn’t shown here, but it’s the top failure mode in the current DMA controller versions Claude has been producing. Verify in code that the OAM PUT operation writes to `$2004` behavior, not “direct index write”.

---

## One hard question you must answer

Does your DMA controller ever read `$2007` as the repeated read during DMA idle cycles, and does that actually increment the VRAM address multiple times?

If the answer isn’t “yes, and it’s covered by a test,” you don’t have accuracy — you have vibes.

---

