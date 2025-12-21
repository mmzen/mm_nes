PPU DMA code review
---

Good—this is the right architectural move (turn $4014 into a **signal**, not a “do the DMA right here” trap). But you still have a couple of correctness landmines and one design smell that will hurt you later.

## 1) **Your `read_byte()` address decode is wrong**

You expose the device at virtual range `(0x4014, 0x4014)`, but then you do:

```rust
fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
    let value = match addr {
        0x00 => self.last_transfer_addr,
        _ => unreachable!()
    };
    Ok(value)
}
```

That’s inconsistent. Depending on how your bus maps devices, `addr` might be:

* the **absolute CPU address** (0x4014), or
* an **offset** (0x00) relative to the device.

Your `write_byte(&mut self, _: u16, value: u8)` ignores `addr`, so you “get away with it” there. But the read will randomly panic if the bus passes absolute addresses.

**Fix (pick one and be consistent across all devices):**

* If bus passes absolute addresses: match `0x4014`
* If bus passes offsets: keep `0x00`, but then your `get_virtual_address_range()` usage must guarantee offsetting.

Brutal truth: **this is a “works until it doesn’t” bug**—and it will show up as a panic in debug builds or wrong behavior in release builds if optimized.

## 2) “Reading returns last written value” is a shaky claim

On the NES, reads from many APU/IO registers behave like **open bus** or return something device-specific; $4014 is typically treated as write-only, and what you get on read is not something games rely on.

If your Bus already handles open-bus, you shouldn’t be inventing deterministic semantics here. Your own comment even contradicts itself:

> “open bus behavior is handled at the bus level”
> but then you return `last_transfer_addr`.

That’s not open bus.

**Fix options:**

* Best: make `read_byte` return an error or a bus-defined open-bus value, and have bus layer provide it.
* Acceptable: return `last_transfer_addr`, but then update docs and be clear it’s your emulator behavior, not hardware truth.

Right now it’s neither.

## 3) You’re missing the timing contract: *when* does DMA start relative to the $4014 write?

Your scheduler does:

* check `dma_start_page` at the start of `step_master_cycle()`
* start DMA immediately in that same cycle before bus arbitration

But the $4014 write itself happens during a CPU bus op. You need to be precise about whether OAM DMA begins:

* on the **next** CPU cycle after the write, or
* “immediately” after write completion, or
* aligned to APU phase boundaries

Different emulators get this wrong and still “run games,” but test ROMs won’t forgive you.

**What to do:**

* Decide the exact contract: “writing $4014 sets the latch; the scheduler samples it at the start of the *next* master cycle.”
* Enforce it by ensuring the write that sets the cell happens during the previous cycle’s bus op, and you don’t read+clear it until the next cycle. (Your current flow likely does this, but it’s implicit and fragile.)

Add a test:

* CPU writes $4014 on cycle N
* First DMA halt attempt begins on cycle N+1 (not N)

If you can’t write that test, you don’t actually know your timing.

## 4) Using `Rc<Cell<Option<u8>>>` is fine, but you’re relying on “single write before read”

What happens if the CPU writes $4014 twice before the scheduler samples the cell? Hardware behavior: second write re-triggers DMA (and on real hardware you’re basically doing dumb things, but it’s still defined enough).

Your current design: the second write overwrites the first page silently. That’s probably fine, but be explicit.

**If you want robustness:**

* Use a small queue (or store a “pending count + last value”)
* Or define “last write wins” (and document it)

## 5) Logging in `initialize()` is noise

`info!("initializing PPU DMA (cycle-accurate mode)");`

This will spam logs on every init/reset/build path. It’s not useful after day 1.

Make it `debug!` or remove it.

---

# The one big integration issue you still must fix (ties to my earlier review)

Even with this PpuDma signal module, your scheduler currently halts CPU whenever `dma_controller.is_active()`—which includes `PendingHalt`. So **as soon as $4014 is written and you call `start_oam_dma()`, the CPU will freeze immediately**, even if you intended PendingHalt to “wait for a read cycle.”

That’s not a PpuDma bug, but this module makes it easier to reproduce.

---

## What I’d tell Claude to change in this file immediately

1. Fix address decode:

```rust
match addr {
    0x4014 => self.last_transfer_addr, // if absolute
    _ => unreachable!()
}
```

(or offset, but consistent)

2. Stop lying about open bus. Either:

* return last value and document “emulator behavior”, **or**
* delegate to bus open-bus handling (preferred).

3. Add a comment specifying the scheduler sampling edge:

* “The scheduler samples `dma_start_page` at the start of the next master cycle.”

