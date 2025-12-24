DMA feedback update
---

Good — this is a **real improvement**. You fixed the two nastiest conceptual bugs from the first DMA rewrite:

* **OAM is now truly phase-gated** (WaitGet/WaitPut only, no “Get/Put” states that can drift).
* **Pure intent + commit** is the correct architecture. No more “advance then rollback” garbage.

That said: you still have **one hard correctness hole** that will bite you the second you hit “address-less intent”.

---

## 1) Your scheduler can still capture `None` for the “halted read address” (fatal under some addressing modes)

In `NesConsole::step_master_cycle` you do:

```rust
let cpu_bus_intent = self.cpu.borrow().get_pending_bus_operation();
let pending_read_addr = if !cpu_is_writing { cpu_bus_intent.address } else { self.last_cpu_bus_address };
...
dma_controller.step_cycle(cpu_is_writing, pending_read_addr, apu_phase)
```

**Problem:** Your CPU’s `get_pending_bus_operation()` / `compute_pending_bus_intent()` sometimes returns `address: None` for read cycles (you even had comments “Complex to compute” in CPU code).

So you can end up halting on a read cycle where `pending_read_addr == None`.

Then in DMA controller, on halt success:

```rust
self.cpu_halted_addr = pending_read_addr; // might be None
```

And later, during stalled “no DMA bus op” cycles, `CpuRepeat` won’t happen because:

```rust
if self.is_cpu_stalled() {
    if let Some(addr) = self.cpu_halted_addr {
        return (Read(addr), CpuRepeat);
    }
}
```

**Result:** you lose the repeated-read side effects you explicitly built this machinery to preserve (e.g., $2007 increment behavior), and your “one bus op per cycle” degenerates into “sometimes zero” when stalled.

### Fix it (non-negotiable)

In the console, change:

```rust
let pending_read_addr = if !cpu_is_writing {
    cpu_bus_intent.address
} else {
    self.last_cpu_bus_address
};
```

to:

```rust
let pending_read_addr = if !cpu_is_writing {
    cpu_bus_intent.address.or(self.last_cpu_bus_address)
} else {
    self.last_cpu_bus_address
};
```

Even better: **fix it at the source** by making CPU bus intent always produce a concrete address on reads (including the “complex” modes). Your DMA correctness should not depend on “maybe address, maybe not”.

If you don’t fix this, you will chase phantom timing failures forever.

---

## 2) Your “last CPU bus address” tracking is the right direction, but you’re still trusting CPU to report it

You now track:

```rust
if result.memory_read || result.memory_write {
    self.last_cpu_bus_address = result.address;
}
```

That is good **only if** `CpuCycleResult.address` is always populated on bus activity. If your CPU sometimes reports `memory_read=true` with `address=None`, you’ll keep a stale address and your “fallback” will still be wrong.

### Fix

Make it:

```rust
if (result.memory_read || result.memory_write) && result.address.is_some() {
    self.last_cpu_bus_address = result.address;
}
```

And again: the real fix is making CPU always report an address for any bus op.

---

## 3) Scheduler/DMA “one bus op per cycle” invariant: your logic is coherent now

This part is solid:

* You step DMA every cycle if active.
* You decide “DMA used bus” by `BusOp != None`.
* You only step CPU if not stalled and DMA didn’t use bus.

That’s the right enforcement point for “exactly one bus master”.

The one caveat: if DMA is stalled and returns `BusOp::None` and `cpu_halted_addr` is `None`, you violate your “one bus op” model silently (ties back to #1).

---

## 4) The updated DMA controller is *much* better — but check one leftover comment-to-code mismatch

You still have this doc in `is_cpu_stalled()`:

> OAM DMA: `Halt`, `WaitGet`, `Get`, `WaitPut`, `Put`

But you removed `Get/Put`. Not a correctness issue, but if you leave this, someone will reintroduce the old broken model later “because docs say so.”

Fix the comment.

---

## 5) DMC sequencing is acceptable as long as phase toggling is truly once-per-master-cycle

You’re using:

* DMC: PendingHalt → Halt → Dummy → Align? → Read
* Align decision uses `next_phase`

That’s fine given your console toggles phase exactly once after the bus op. Keep the console ordering exactly as is, or you’ll break DMC reads landing on PUT.

If you ever change the scheduler order, add a big invariant comment:
**“apu_phase passed into DMA is the phase of the bus op for this master cycle.”**

---

# Verdict

### NesConsole: **close, but still vulnerable**

Your `last_cpu_bus_address` change was the right move. But the current logic still allows `None` to leak into DMA capture. That’s a timing correctness hole.

### DMA controller: **good architecture now**

The “pure intent + commit” rewrite fixed the earlier structural mistakes. This is the right foundation.

---

# What I want you to do next (ruthless priority)

1. **Patch `pending_read_addr` to never be None when you’re trying to halt**
   Use `.or(self.last_cpu_bus_address)` immediately.

2. **Make CPU bus intent always provide concrete read addresses**
   Stop returning `None` in `compute_executing_bus_intent()` for “complex to compute”. Compute it. That’s literally what this system needs.

3. **Harden last_cpu_bus_address updates**
   Only overwrite it when `result.address.is_some()`.
