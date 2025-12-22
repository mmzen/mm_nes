NES_BUS feedback
---

This bus is **good enough to support accurate DMA/open-bus**, but it has **two correctness bugs** and **one architectural trap** that will absolutely create hard-to-debug timing mismatches later.

I’ll keep it tight.

---

## What you did right

### 1) You actually model open bus as a shared latch

`data_bus: Rc<Cell<u8>>` updated on reads and writes is the right mechanism. This is what makes `$4014` reads, `$2007` weirdness, etc. even possible.

### 2) Writes update the bus

```rust
self.data_bus.set(value);
```

That’s correct: CPU drives the data bus on writes.

### 3) `$4015` exception is intentional

You’re trying to model “internal bus vs external bus” split for APU status reads. That’s directionally correct.

---

## Bug #1: `trace_read_byte` does not update open bus (this will break DMA conflict debugging)

Your normal `read_byte` updates the bus (except `$4015`). But your tracer:

```rust
fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
    let value = memory.borrow().trace_read_byte(effective_addr)?;
    Ok(value)
}
```

does **not** update `data_bus`. That means any trace-driven tool, debugger, or test harness that uses `trace_read_byte` will observe behavior that diverges from real bus behavior.

If you ever run DMA cycle tests using trace reads (common), you’ll get false positives/negatives.

**Fix:**
Either:

* Make `trace_read_byte` update `data_bus` exactly like `read_byte` (except for `$4015`), **or**
* Rename it to `peek_byte` and forbid its use for anything timing/bus-accurate.

Right now it’s a footgun.

---

## Bug #2: `read_word` updates the bus twice and that can be wrong for some side effects

You implemented:

```rust
let low = self.read_byte(addr)?;
let high = self.read_byte(addr.wrapping_add(1))?;
```

That’s a reasonable approximation, but it has consequences:

* It triggers **two separate device reads with side effects**, not a single atomic “word read.”

Most 6502 “word reads” are not real bus operations; they’re sequences of byte reads that happen in specific cycles, sometimes with dummy reads and page-wrap quirks. Your CPU core should already be doing byte-level reads per cycle. If `read_word` is used anywhere in actual execution, you just reintroduced instruction-boundary behavior and side-effect duplication.

**Fix:**
Make `read_word`/`write_word` **test-only helpers** or prohibit them in cycle-accurate execution paths. If they’re used in the CPU core, rip them out and model word fetch at the CPU microcycle level instead.

---

## Architectural trap: address mirroring via `addr & (size-1)` is too generic and will silently break devices

This line:

```rust
let effective_addr = addr & (device.borrow().size() - 1) as u16;
```

This assumes:

* size is a power of two
* mirroring is correct as a simple mask

That’s true for some devices (WRAM mirrors), but false or dangerous for others:

* PPU registers are mirrored every 8 bytes across `$2000–$3FFF` (mask `0x2007` logic), not “size-based” unless the device is built exactly for that.
* Some mappers / cartridge registers have non-power-of-two mapping patterns.
* Your `OpenBus.size()` returns 1 → `effective_addr` is always 0, which is fine, but the same trick on a non-power-of-two device becomes nonsense.

**Fix:**
Enforce the invariant instead of assuming it:

* Add a debug assert: `size.is_power_of_two()`.
* Or make each device supply an `effective_addr(addr)` mapping function (cleaner, more accurate long-term).

Right now a single device with size=3 or size=0 will produce garbage masking.

---

## About your `$4015` special case

This line:

```rust
if addr != 0x4015 { self.data_bus.set(value); }
```

That’s a decent approximation for “$4015 doesn’t drive the external bus,” but be careful: `$4015` read also clears frame interrupt flags, and other APU reads/writes can have mixed internal/external behavior. You’re doing the minimum; fine.

But don’t stop here if you want very accurate DMC conflict behavior — eventually you’ll need “internal bus” modeling, not just one exception.

---

## Bottom line

This bus is *close*, but if you want accuracy you must fix:

1. **`trace_read_byte` must not diverge from `read_byte` open bus semantics**
2. **`read_word` must never be used by the cycle-accurate CPU path**
3. **stop silently masking with `(size-1)` without enforcing power-of-two**

If you implement those, your open bus model becomes trustworthy and your DMA timing work won’t be undermined by the bus layer.
