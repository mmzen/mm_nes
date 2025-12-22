DMU feedback analysis
---

---

## What’s solid now

### 1) The core invariant is finally real: one bus op per CPU cycle

Your scheduler enforces:

* DMA can generate a bus op (`Read/Write`)
* If DMA used the bus, CPU must not step
* If DMA did not use the bus and CPU isn’t stalled, CPU steps

That’s the right structure. This is what stops “ghost reads” and accidental double side effects.

### 2) Correct capture of “halted read address”

You pass `pending_read_addr` down and the DMA controller captures it **at the exact moment halt succeeds**. That’s the right behavior for RDY repeated reads, and it’s the prerequisite for correct `$2007/$4016/$2002` side effects.

### 3) OAM PUT goes through the bus to `$2004`

That’s critical. It ensures:

* OAMADDR behavior is respected
* writes can be ignored during rendering if your PPU does that
* open bus is updated by actual bus writes, not some fake helper

### 4) GET/PUT gating is explicit and caller-driven

The DMA controller no longer maintains its own phase. Passing `apu_phase` into `step_cycle` makes the phase semantics explicit and testable.

---

## The accuracy gaps you still have

### A) OAM DMA is missing the **mandatory alignment cycle logic**

Right now OAM DMA does:

* PendingHalt → Halt (no bus op)
* Halt → WaitGet
* then Get only when `current_phase.is_get()`

This can produce *some* natural alignment, but it’s not guaranteed to match hardware’s “513 vs 514” structure because hardware has:

* **Halt cycle** (no DMA access)
* **Optional extra alignment cycle** if the next cycle is PUT (so the first DMA memory read occurs on GET)
* Then 256 GET/PUT pairs

In your current model, there is no explicit “alignment cycle” state. You’re relying on `WaitGet` and phase gating to “wait out” a PUT cycle, which *can work*, but only if the transition timing is correct relative to `apu_phase` toggling and the halt cycle.

**Risk:** you’ll get an off-by-one in the 513 vs 514 cases depending on initial phase and whether halt was delayed.

**Fix (recommended):**
Add an explicit OAM “Align” state:

* `Halt → Align`
* `Align` consumes exactly one cycle **only if** `current_phase` at the next cycle is PUT (or if first needed op would land on PUT)
* then `Align → WaitGet`

If you want to keep phase-gated waiting, you still need a deterministic way to count/validate. Otherwise your tests will pass “by accident”.

---

### B) DMC DMA sequencing is incomplete (it’s missing the **Halt → Dummy → (Align) → Read** timing constraints)

You implemented DMC phases, but look at the actual transitions:

* You move `PendingHalt → Halt`
* Then in `advance_dmc_no_bus_phases(next_phase)` you do:

    * Halt → Dummy
    * Dummy → Align or Read depending on next_phase
    * Align → Read

This is close, but it contains a subtle correctness question:

**When exactly does the transition happen relative to the cycle?**
Your transitions happen at the end of `step_cycle()` (Step 5). That means:

* the cycle where `dmc.phase == Halt` is a no-bus cycle (good)
* next cycle sees `Dummy` (good)
* alignment decision uses `next_phase` computed from `current_phase` of the cycle you’re currently in

This is *probably correct*, **as long as the caller’s `apu_phase` is “phase for this CPU cycle before bus arbitration”** and is toggled after the bus op. Your console does toggle after. Good.

But: you currently allow DMC `Read` only when `current_phase.is_get()`. If you end up in `Read` on a PUT cycle (possible if the Align decision is off by one), you’ll stall in `Read` with `BusOp::None` until a GET arrives—creating an extra cycle not always present in hardware.

**Fix (recommended):**
Make DMC `Read` state *only* entered when you know the next cycle will be GET. Your Dummy→Align/Read decision is supposed to guarantee that; add an assert in debug builds:

* if `dmc.phase == Read` then `current_phase.is_get()` must be true, else panic in debug.

That catches drift early.

---

### C) DMC scheduling contract is risky (APU “authoritative” is fine, but you must prove it)

Your console does:

```rust
let dmc_dma_request = self.apu.borrow().needs_dmc_dma();
if let Some(addr) = dmc_dma_request {
    self.dma_controller.request_dmc_dma(addr);
}
```

This implies:

* APU decides **which CPU cycle** to request on (load vs reload timing)
* DMA only executes the sequence once requested

That’s a valid design, but it means your timing accuracy lives in APU. If APU is even 1 cycle off, the emulator will be wrong in exactly the DMC conflict edge cases you’re chasing.

**Non-negotiable requirement:** you need tests that prove APU requests on the correct cycle boundary:

* Load DMA: GET of the 2nd APU cycle after enabling
* Reload DMA: request aligned to a PUT cycle start
* plus “halt cannot succeed on writes”

If you don’t have those tests, you don’t have accuracy—you have a “story”.

---

### D) CPU repeated read behavior depends entirely on bus side effects

In `CpuRepeat` you do:

```rust
let _ = self.bus.borrow().read_byte(addr)?;
```

That’s correct **only if**:

* `Bus::read_byte($2007)` increments VRAM address
* `Bus::read_byte($4016/$4017)` clocks controller
* `Bus::read_byte($2002)` clears vblank flag
* open bus is updated appropriately (if modeled)

If your bus or devices implement “peek” semantics anywhere, you must ensure DMA uses the side-effecting path.

---

## Architectural problems you should fix soon

### 1) The DMA controller still holds `ppu: Rc<RefCell<D>>` but doesn’t use it

You’re now writing `$2004` via bus. Good. So this `ppu` field is now dead weight and a future confusion point.

**Fix:** remove it from `DmaController` unless you still need it for something real.

### 2) `request_dmc_dma` should probably return a boolean (“accepted”) for debugging

Right now it silently ignores duplicates. That’s okay, but it hides bugs.

---

## Quick sanity checks to add (you’ll thank yourself)

Add debug assertions:

1. **No CpuRepeat during PendingHalt**
   If `!is_cpu_stalled()` then arbiter must not produce `CpuRepeat`.

2. **DMC Read must happen on GET**
   If `winner == DmcRead`, assert `current_phase.is_get()`.

3. **OAM PUT must happen on PUT**
   If `winner == OamPut`, assert `current_phase.is_put()`.

These are cheap and will catch 90% of timing drift as soon as it’s introduced.

---

## Bottom line

You’ve moved from “DMA is kind of modeled” to “DMA is a real arbiter.” That’s the biggest leap.

What’s left is **tightening the phase-alignment behavior** so 513/514 and 3/4 cycle cases are deterministic and match hardware, and making sure the APU’s DMC request timing is actually correct and tested.

