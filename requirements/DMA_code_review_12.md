DMA feedback update
---

You made real progress. This is no longer “DMA sprinkled on top.” You’re converging on an architecture where **CPU exposes per-cycle bus intent**, DMA arbitration happens **before** execution, and you enforce **one bus op per master cycle**. That’s the only road to legitimacy.

But you still have a few correctness traps that will absolutely surface in functional tests. Here’s the blunt review of the *changes* across **nes_console.rs**, **dma_controller.rs**, and the **CPU bus-intent work**.

---

## High-level: what improved

### ✅ Scheduler correctness improved

* You now compute `pending_read_addr` from **CPU’s pending bus op**, not “last read.” That’s the right model for DMA stalls and repeated reads.
* You enforce the invariant:

    * **if DMA used the bus OR CPU is stalled → CPU must not step**
    * otherwise CPU steps
* You added `debug_assert_eq!(cpu_result.halted, expected)` — good. This catches silent double-stepping.

### ✅ DMA controller no longer mutates in “intent” phase

* You moved OAM GET/PUT to **pure intent** + **commit after win**. That removes rollback bugs and makes arbitration sane.

### ✅ CPU now provides “bus intent” per cycle

* This is the biggest step. Without this, cycle accuracy is theater.

---

## The big remaining problems (these will bite you)

## 1) Your “last CPU bus address” model is still wrong in key cases

In `nes_console.rs` you track `last_cpu_bus_address` when CPU does a read or write and exposes an address.

That helps, but the repeated read during DMA stalls is not “last bus address” in all cases. On the real 6502/NES:

* the bus is often driven by **the current microcycle’s address**, which may be a dummy read or internal fetch address
* during DMA stalls, the “repeated read” address is tied to the halted read cycle (you’re trying to capture that — good)
* but if `CpuBusIntent.address` is `None`, you’re falling back to historical last bus address, which is a guess

**The trap:** your CPU’s `get_pending_bus_operation()` returns `default()` for many cycles (especially when state hasn’t been computed yet), and you’ll silently degrade into “random fallback address.” That will fail tests involving side effects on repeated reads ($2007 increments, controller strobe reads, etc.).

**Fix direction (not optional):**

* Make `CpuBusIntent.address` **never None** during any cycle where the real CPU would touch the bus.
* If you truly have internal cycles with no bus op, then you must:

    * represent that explicitly as `op: None`, and
    * decide whether DMA can halt on that cycle (usually: no).
      Right now `None` is overloaded as “unknown,” which is deadly.

---

## 2) DMC DMA is still too optimistic: completion is immediate

In `dma_controller.rs` you still do:

```rust
self.dmc.sample = Some(value);
result.dmc_sample = Some(value);
self.dmc.phase = DmcDmaPhase::Idle;
```

So DMC fetch ends instantly on the read cycle.

Even if the read itself is correct, the stall/release timing is not guaranteed to match hardware. And DMC + OAM overlap edge cases are where most “cycle accurate” emulators die.

**Fix direction:**

* Add a post-read phase, even if it’s 0 cycles on your current belief, so you can adjust once tests tell you you’re off.
* Or encode “CPU resumes next cycle” explicitly rather than implicitly.

---

## 3) The GET/PUT phase abstraction is still unproven and can drift

You toggle `apu_phase` every master cycle in the console:

```rust
self.apu_phase = self.apu_phase.toggle();
```

This assumes the GET/PUT alternation is tied to **elapsed CPU clocks**, not **CPU bus usage**.

That can be fine if you define phase as “master cycle parity,” but then call it that. Don’t call it APU phase unless you’ve verified it matches APU/DMC timing edges.

**Key integration risk:**

* During DMA stalls, you continue toggling phase — does real hardware effectively “advance the phase” while CPU is stalled? Depending on what this phase represents, this might be right or wrong. You have no proof yet.

**Fix direction:**

* Rename it to `cycle_parity` if that’s what it is.
* Make it derived from `master_cycles & 1`, not a mutable variable. Mutable toggles are drift bugs.

---

## 4) OAM DMA start alignment is still wrong unless you capture the exact $4014 write boundary

You trigger DMA with a `Cell<Option<u8>>` set by the $4014 write.

But you aren’t capturing the **phase/parity at the actual write completion**, which determines 513 vs 514.

You wrote comments that imply “alignment happens naturally,” but your OAM state machine currently:

* waits for a non-write cycle to enter Halt
* then does 1 halt cycle
* then alternates GET/PUT with phase gating

This will approximate length but can be off by one unless the phase at trigger time is correct.

**Fix direction:**

* store not only page, but also **phase/parity at trigger time** (or master cycle count when written).
* use that to set initial alignment deterministically.

---

## 5) Your CPU bus-intent generator has correctness holes

This is the part most people underestimate. You can’t half-model bus intent.

A few red flags from the CPU snippet:

### a) Implicit/Accumulator bus intent is suspicious

You return:

```rust
address: Some(pc.wrapping_add(1))
```

for implicit/accumulator dummy reads. That might match many instructions, but the exact dummy read address for some ops isn’t always PC+1 in all cases, especially across interrupts, BRK, etc. If your execution uses different dummy addresses, intent must match execution.

**Hard rule:** intent must match what `step_cycle()` will actually read/write **that same cycle**, or DMA arbitration becomes fiction.

### b) InterruptSequence intent comments vs actual sequence

You’re modeling interrupt cycle 1 as a dummy read in FetchOpcode. That’s fine, but your intent method for InterruptSequence assumes cycle numbers 2-7. Make sure `cycle_state` is consistent. Any off-by-one there will break DMA halt eligibility on interrupt entry.

### c) RMW: you now do real bus writes during cycle execution

Good. But you also have helpers (like `rmw_overwrite`) that in cycle-accurate mode store pending values rather than writing. That creates a risk:

* your “bus intent” says “write this cycle”
* your execute_cycle does or does not actually write depending on operand type

That must stay consistent or arbitration breaks.

**Fix direction:** assert internally in CPU:

* if bus intent says Write(addr), the execution path **must** perform that `write_byte(addr, ...)` on that cycle for cycle-stepped mode.

---

## Smaller issues (still worth fixing)

### 6) `needs_dmc_dma()` called every cycle, duplicates ignored: ok, but noisy

You log when rejected — good — but you’re still calling it unconditionally. That’s fine if `needs_dmc_dma()` is cheap. If it’s not, add a cheap check.

### 7) `debug_assert!` in DMA is great, but you need one for “one bus op”

You enforce in the scheduler, but you should also assert:

* if DMA returns `BusOp::Read/Write`, CPU must not have stepped
* and if CPU stepped, DMA must have returned `BusOp::None`

You already do this indirectly. Add one more assert explicitly based on `cpu_result.bus_op` if you expose it.

---

# The one thing you must do next (or you’ll stay in “plausible” land)

Add a per-cycle trace at the scheduler level:

* master cycle index
* cpu intent: (R/W/None, addr)
* dma winner + bus_op
* cpu stepped? (halted)
* apu_phase/parity
* last_cpu_bus_address
* dmc phase + oam op/byte_index

Then run:

* `ppu_vbl_nmi` suite you already have
* plus DMC timing ROMs
* plus one or two real games with DPCM + sprite DMA overlap (or crafted tests)

Until you can diff this trace against a reference emulator (Mesen), you don’t know if you’re right — you only know you’re coherent.

---

## Bottom line

You’re doing the right kind of work now. But:

* **bus intent must be perfect**, not “best effort”
* **phase/parity must be a derived invariant**, not a toggled variable
* **DMC completion timing is still likely off**
* **OAM start alignment still needs trigger-time parity**

Fix those, and you’re no longer “Claude-built emulator code.” You’re building something that can stand up to adversarial test ROMs.
