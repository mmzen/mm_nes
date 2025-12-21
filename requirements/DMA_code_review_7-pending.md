DMU update feedback
---

Good: Claude actually implemented the core scheduler fix in the **right direction** (`is_cpu_stalled()` and CPU keeps running during `PendingHalt`). That was the big architectural hole.

Now the brutal part: he *still* left a couple of correctness and design liabilities that will absolutely bite you when you start running timing ROMs or when someone refactors.

---

# ✅ What’s solid now

### 1) Scheduler decision split is right

You now:

* always call `dma_controller.step_cycle(...)` when `is_active()` (so PendingHalt advances)
* only halt CPU when `is_cpu_stalled()`

That’s exactly what you needed.

### 2) Builder wiring for shared data bus

You correctly threaded `data_bus` into `PpuDma::new_with_dma_signal(..., data_bus)` and you’re cloning the same cell around. Good. That makes open-bus globally consistent *if* `NESBus` updates it correctly.

---

# 🔥 What’s still wrong / sloppy

## 1) **You still have two bus ops per cycle in `PendingHalt`** (architecturally)

Read this carefully.

When DMA is active but not stalling (PendingHalt), you do:

* `dma_controller.step_cycle(...)` → **may perform a bus op** (e.g. CpuRepeat if your arbiter allows it, or future changes)
* then `cpu.step_cycle()` → **CPU performs a bus op**

Right now, in `PendingHalt`, `step_cycle()` returns no bus op, so you “get away with it.” But your architecture now *allows* two bus transactions in one master cycle if someone tweaks the DMA controller later.

You claimed “exactly one bus op per CPU cycle” as a global invariant. Your scheduler currently does not enforce it. It enforces it only when CPU is stalled.

**Fix the invariant explicitly:**

* When CPU is not stalled, DMA must be allowed to **advance only non-bus state**, or return `BusOp::None` guaranteed.
* Or, simplest: change scheduler ordering so **only one actor executes bus ops per cycle**:

    * If DMA is active, let DMA decide bus op.
    * If DMA returns `BusOp::None`, then CPU executes bus op.
    * If DMA returns `Read/Write`, CPU must not execute.

Right now you execute CPU regardless of whether DMA used bus (because you decide based on `is_cpu_stalled()` not on `bus_op`).

### Minimal robust change (do this):

After computing `dma_result`, add:

* if `dma_result.bus_op != None` then CPU must not execute this cycle

That protects your “one op” rule even during PendingHalt if DMA ever does something.

---

## 2) `DmaStepResult.cpu_halted` is still wrong in the controller

In `DmaController::step_cycle()` you still set:

```rust
result.cpu_halted = self.is_active();
```

That’s false for PendingHalt by your own definition. It should be:

```rust
result.cpu_halted = self.is_cpu_stalled();
```

Yes, you’re not currently using it in the scheduler (you build your own `CpuCycleResult`), but leaving wrong signals in structs is how bugs creep back.

**Fix it now.**

---

## 3) `calculate_dmc_dma_cycles()` returning “3” is unacceptable

It’s still there:

```rust
pub fn calculate_dmc_dma_cycles(...) -> u8 { 3 }
```

This is a loaded gun. Someone will call it (or old code still does), and you’ll get “timing seems off” reports for weeks.

**Fix:**

* Remove it, or
* Make it `panic!()` / `unreachable!()` with a message, or
* Return `Option<u8>` and always `None`

Anything but “3”.

---

## 4) The DMC “PendingHalt if CPU already halted” condition is now inconsistent with your scheduler

In DMC PendingHalt you do:

```rust
let cpu_already_halted = matches!(self.oam.op, Halt | WaitGet | Get | WaitPut | Put);
if !cpu_is_writing || cpu_already_halted { ... }
```

But now the scheduler can have DMA active while CPU still running (PendingHalt). So “already halted” should really be “will be stalled this cycle” (which you can compute with `is_cpu_stalled()`), not “OAM op matches these states.”

It might still be correct, but it’s brittle.

**Fix:**

* Replace ad-hoc `cpu_already_halted` with `self.is_cpu_stalled()` (or an equivalent internal check).

---

## 5) You’re mixing “CPU pending read address” with “CPU executed” timing

You pass `pending_read_addr` (from “what CPU *intends* to do”) into DMA before CPU actually executes. That’s fine if your CPU core’s `get_pending_bus_operation()` is accurate.

But if the CPU changes its bus op after some internal state advance (e.g., weird edge micro-ops), you will capture the wrong address at the halt boundary.

Not saying it’s wrong, saying: **this must be validated.** If `get_pending_bus_operation()` is even slightly approximate, repeated read behavior will be off.

**Action:** add a test that hits known side-effect registers ($2007, $4016) during DMA stall and checks increments / latches.

---

# What I would tell Claude to fix next (in order)

1. **Enforce one bus op per cycle** in the scheduler:

    * If `dma_result.bus_op != None`, do not run `cpu.step_cycle()` that cycle.
    * Only run CPU when DMA returns `BusOp::None` (and CPU isn’t stalled).

2. **Fix `DmaStepResult.cpu_halted`** to use `is_cpu_stalled()`.

3. **Delete or hard-fail** `calculate_dmc_dma_cycles()`.

4. Replace `cpu_already_halted` with a single internal “stalled” predicate to reduce future drift.

---

# Quick sanity check of your new flow

Right now, your logic does this:

* DMA active but PendingHalt:

    * DMA step runs (no bus op)
    * CPU step runs (bus op happens)
      ✅ OK today, but fragile.

* DMA stall:

    * DMA step runs (maybe bus op)
    * CPU step does not run
      ✅ OK.

Make it explicit so it stays OK after refactors.

---

