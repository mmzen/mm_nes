DMA controller update feedback
---

---

## 1) Critical correctness failures

### A) OAM DMA alignment logic is wrong (and your comment reveals it)

You wrote:

> “Fixed OAM DMA alignment logic (1 idle for even, 2 for odd)”

and implemented:

* always do cycle 0 idle
* do cycle 1 idle only if `started_on_phase.is_put()`

That yields 1 or 2 idle cycles depending on GET/PUT. That’s fine *as a count*, but the **reasoning is wrong**: OAM DMA alignment is not “started on PUT → add an extra idle.” It is:

* **halt cycle** occurs when RDY first successfully halts CPU on a read
* then **optional** alignment so first DMA source read occurs on GET

Your code has **no concept of halt attempt failure on CPU write cycles**, and no “halt cycle” that depends on the CPU’s current bus direction. You just start DMA and unconditionally do “idle cycles”.

If the CPU writes to $4014 during an instruction that ends with a write cycle pattern, on real hardware the OAM DMA halt can be delayed. Your controller cannot represent that.

**Verdict:** OAM DMA start/idle is still instruction-era simplified logic.

---

### B) OAM DMA read/write phases ignore GET/PUT completely after alignment

After the initial idle(s), you just alternate `read_phase` boolean each call:

```rust
if self.oam_dma.read_phase { read_byte(...) } else { ppu.dma_write(...) }
```

But on real hardware, **reads must happen on GET cycles and writes on PUT cycles**. You’re assuming that because you started aligned, alternating will always match GET/PUT. That only holds if nothing ever disturbs the cadence.

But you *do* disturb it: DMC DMA can steal a GET cycle. When it does, OAM DMA must “wait for GET,” and the next PUT becomes an idle/no-bus cycle (realignment). Your controller does not have a “waiting for GET/PUT” state. It has only “read phase / write phase”.

So any overlap will desync your intended alternation and the APU phase, and your model will still perform reads on PUT cycles.

**Verdict:** OAM DMA is not actually constrained by GET/PUT.

---

### C) DMC DMA timing is made-up (halt counts 1–3 are not how it works)

This is the biggest lie in the file:

```rust
if cpu_is_writing { Halt(3) }
else { Halt(2) }
...
// Last halt cycle done, go to read
self.dmc_dma.phase = Read;
```

Real DMC DMA is **not** “halt N times then read”.
It is:

* attempt to halt; halt only succeeds on CPU read
* **halt cycle** (no DMA bus access)
* **dummy cycle**
* optional alignment to GET
* GET read from DMC address

Also: reload vs load scheduling differs (PUT vs GET of “2nd APU cycle after enabling”). Your controller does not represent either load or reload scheduling — it just “requests DMA” and begins immediately with a guessed halt count.

**Verdict:** DMC DMA is fundamentally incorrect.

---

### D) You violate the bus model: two reads in one CPU cycle

This is fatal:

```rust
} else if self.dmc_dma.active && self.oam_dma.active {
    let dmc_result = self.step_dmc_dma()?;
    let oam_result = self.step_oam_dma()?;
}
```

That is literally “do two bus operations in one CPU cycle”.

Even if you’re trying to represent “DMC halt cycles happen while OAM DMA continues,” you cannot do it this way. On hardware, **only one external bus master exists per CPU cycle**. If OAM DMA is performing a GET read from memory, the bus is occupied. The CPU can’t also perform a repeated read. The DMC unit can’t also do another read.

What really overlaps is **stall time**, not “extra bus cycles”. In the overlap case, DMC “halt/dummy/align” can coincide with OAM transfer cycles because they are *no-bus* from the DMC perspective, but the CPU is already stalled and the bus is being used by OAM. Your code instead performs additional reads (conflict reads) on top of OAM activity.

That will:

* create extra $2007 increments
* extra $4016 clocks
* extra $2002 clears
* everything will drift, and some tests will “pass” only because you accidentally emulate a different broken behavior.

**Verdict:** This cannot be considered cycle-accurate.

---

### E) “Conflict reads” are mis-modeled

You’ve conflated two distinct things:

* **CPU repeated read during RDY stall** (CPU continues to put its address and read it on cycles where DMA isn’t using the bus)
* **DMC conflict address behavior** (internal decode weirdness where APU/joypad reads can be affected)

You model DMC halt as:

```rust
read_byte(conflict_addr)
```

But on real hardware, during DMC halt/dummy/align cycles, the bus is typically showing **CPU’s repeated read**, *not* necessarily “conflict address.” The “conflict address” effect is about internal decode when the DMC address mux + CPU high bits cause APU register reads to happen. That’s not what you implemented.

**Verdict:** You’re inventing a mechanism and labeling it “conflict”.

---

## 2) Secondary issues that will bite you

### Using `unsafe` to read bus is a smell

```rust
let bus_ptr = self.bus.as_ptr();
self.oam_dma.read_value = unsafe { (*bus_ptr).read_byte(source_addr)? };
```

This is a correctness risk for no real gain. If you hit UB or reentrancy, you’ll never trust your results. Fix the borrow architecture instead.

### `OamDmaState.cycle` is meaningless

It increments, but your logic doesn’t use it for timing after alignment except completion. That’s fine, but it suggests you’re trying to mirror cycle counts rather than enforce actual constraints.

### `Dummy` and `Align` exist but are “currently not used”

Your comment says:

> “Dummy cycle … currently not used in simple model”

That’s exactly the problem: **the dummy cycle is not optional**.

---

## 3) What “accurate” would look like (non-negotiable structure)

You need a DMA controller that, for each CPU cycle, produces **exactly one of**:

* `BusOp::Read(addr)`
* `BusOp::Write(addr, data)`
* `BusOp::None` (meaning CPU repeated read drives bus if RDY stalled; else CPU normal op)

And you need an arbiter that enforces:

* DMC GET read has priority over OAM GET read on a GET cycle
* OAM PUT writes happen only on PUT cycles
* CPU repeated read happens only when DMA does not use the bus that cycle

Your current implementation breaks this by “doing both”.

---

## 4) Minimal set of changes to fix this (ruthless, but precise)

1. **Delete the “step both” overlap branch.**

    * One cycle → one bus op. Always.

2. Replace OAM DMA `read_phase` with `next_op: Get|Put` and gate it by `apu_phase`.

    * If `next_op == Get` and `apu_phase != Get`, then no DMA bus op this cycle.
    * Same for Put.

3. Replace DMC DMA “halt count” with actual phases:

    * `PendingHalt` (retries until CPU cycle is read OR CPU already stalled)
    * `Halt` (1 cycle, no DMA bus op)
    * `Dummy` (1 cycle, no DMA bus op)
    * `Align` (optional 1 cycle if next is not GET)
    * `Read` (GET-cycle bus read from DMC addr)

4. Implement overlap correctly:

    * If DMC is in `Read` and OAM is also requesting `Get`, DMC wins, OAM stays in `Get` waiting.
    * That naturally inserts the required realignment idle PUT next cycle.

5. Remove fake “conflict reads” and implement **CPU repeated reads** properly:

    * When RDY stalled and DMA does not use the bus that cycle, perform `bus.read_byte(cpu_halted_addr)` as the bus op.

Anything else is cosplay.

---

## 5) Bottom line verdict

* **OAM DMA:** not accurate (no halt-attempt failure, not phase-gated after start, broken overlap behavior)
* **DMC DMA:** not accurate (wrong scheduling, missing required dummy/align semantics, invented halt counts)
* **Overlap:** outright wrong (multiple reads per cycle)

If you want “very accurate emulator,” you must stop patching around and commit to a strict “one bus op per CPU cycle” arbiter model with real state machines.

If you paste the CPU’s `get_pending_bus_operation()` and how you model RDY stall, I can tell you exactly where to insert the arbiter and what the DMA controller should output. But as-is, this controller will fail the exact hardware edge cases you’re trying to support.
