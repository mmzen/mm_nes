Good — this fixes the exact ambiguity I called out. The **timing contract is correct**: write `$4014` on cycle N, scheduler sees the latch at the start of cycle N+1, DMA halt attempts begin on N+1. That’s what real hardware does.

Now the review: what’s accurate, what’s still risky, and what you should tighten so this doesn’t regress.

---

## What’s correct and worth keeping

### 1) Start-on-next-cycle semantics are right

Your scheduler reads `dma_start_page` at the start of the master cycle, and `PpuDma::write_byte` only sets the latch. That naturally produces “start on N+1.”

### 2) “Last write wins” behavior is correct

Because you just overwrite the cell:

```rust
self.dma_start_page.set(Some(value));
```

If the CPU writes multiple times before the scheduler samples it, last write wins. That matches what you want.

### 3) `$4014` reads returning open bus is correct enough

Real hardware treats `$4014` as write-only; reading returns whatever is on the data bus. You’re returning `data_bus.get()`.

That is the correct behavior **as long as `data_bus` really is your “external open bus latch”** and is updated on every external read/write.

---

## What’s still wrong / fragile

### A) Your timing contract comment is correct — but the implementation can still violate it if the scheduler samples too late

The PpuDma side is fine. The risk is the scheduler, not shown here:

* If `step_master_cycle()` samples `dma_start_page` **after** doing anything that might call into the bus (or after CPU step), you can accidentally start on N instead of N+1 in edge cases.

**Rule:** sampling must happen at the **very top** of the cycle before any bus op is executed. You’ve said it, so enforce it with a test.

**Test you must add:**

* run one cycle where CPU writes `$4014`
* assert `dma_controller.is_active()` is still false for that same cycle
* assert DMA enters `PendingHalt` at the start of the next cycle

That will prevent future “helpful refactor” regressions.

---

### B) `$4014` read should not mutate bus state, but your design depends on who updates `data_bus`

You return:

```rust
Ok(self.data_bus.get())
```

That’s fine.

But the subtlety: on real hardware, “open bus” is the **last externally driven** value. It’s not a magical register. Your `data_bus` must be updated by the bus layer on:

* every read from devices that drive the external bus
* every write data value (because the CPU drives the bus)
* not by internal-only reads like `$4015` (APU internal) if you’re modeling that split

If `data_bus` isn’t updated consistently, `$4014` reads will be wrong and you’ll miss some conflict/side effects cases.

**Action:** verify `NESBus` updates `data_bus` on all reads and writes (including DMA reads/writes).

---

### C) `trace_read_byte` returning open bus is fine, but can mask debugging

Not an accuracy issue, but be aware: it makes traces look like `$4014` has meaning when it doesn’t. If you later compare against test ROM traces, that’s okay because trace frameworks typically treat open bus as “don’t care.”

---

### D) Address masking comment is probably wrong / too device-specific

You say:

> Bus passes effective_addr = addr & (size - 1). With size=1 this is always 0.

That’s a weird bus design for a memory-mapped device — it works, but it’s easy to break if you ever switch bus behavior. Your match already just returns open bus for everything, so the comment isn’t worth the confusion.

**Action:** simplify:

* ignore `addr` entirely for read, just return `data_bus.get()`.

---

## One more thing you should change (accuracy-adjacent)

### Make `write_byte` also drive open bus

On real hardware, when the CPU writes, the value is on the data bus. Many emulators model this by setting open bus to the written value on *any* write.

Right now, `PpuDma::write_byte` does not touch `data_bus`. That’s okay if your bus layer is already updating `data_bus` with `data_out` for all writes. But if not, you’ll miss open bus correctness.

**Preferred:** bus updates it, not each device.

So: don’t modify `data_bus` here — but confirm the bus does.

---

## Verdict

This module is now **conceptually correct** and consistent with cycle-accurate scheduling:

* It latches a page,
* last write wins,
* scheduler starts DMA on the next cycle,
* reads return open bus.

Your remaining risk is **not here** — it’s whether:

1. scheduler samples at the true start of the cycle, and
2. `data_bus` is truly “external open bus” and updated on all DMA/CPU read+writes.

