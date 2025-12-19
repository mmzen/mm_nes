# PPU Open Bus - Archived Task

**Status**: Completed
**Date Completed**: December 19, 2025
**AccuracyCoin Test**: DUMMY WRITE CYCLE #1 - PASS

---

## Overview

Implemented PPU open bus behavior to pass the AccuracyCoin "DUMMY WRITE CYCLE" test #1: "PPU Open Bus should exist."

---

## Problem

The PPU had no open bus tracking. When reading PPU registers, bits that aren't actively driven by the register should return the last value that was on the PPU's internal data bus (the "open bus" value).

---

## PPU Open Bus Behavior

| Register | Address | Read Behavior |
|----------|---------|---------------|
| PPUCTRL | $2000 | Write-only → returns open bus |
| PPUMASK | $2001 | Write-only → returns open bus |
| PPUSTATUS | $2002 | Bits 7,6,5 valid; bits 4-0 = open bus |
| OAMADDR | $2003 | Write-only → returns open bus |
| OAMDATA | $2004 | All 8 bits valid |
| PPUSCROLL | $2005 | Write-only → returns open bus |
| PPUADDR | $2006 | Write-only → returns open bus |
| PPUDATA | $2007 | All 8 bits valid |

---

## Solution

1. Added `open_bus: Cell<u8>` field to `Ppu2c02` struct
2. Updated `write_byte()` to set `open_bus` on every write to PPU registers
3. Updated `read_byte()` to set `open_bus` with the returned value
4. Updated write-only register reads ($2000, $2001, $2003, $2005, $2006) to return `open_bus.get()`
5. Updated `read_status_register()` ($2002) to return `(status & 0xE0) | (open_bus & 0x1F)`
6. Updated existing test to account for open bus behavior

---

## Files Modified

| File | Change |
|------|--------|
| `ppu_2c02.rs` | Added `open_bus` field; updated read/write methods |
| `tests/ppu_2c02.rs` | Updated test for open bus behavior |

---

## Key Code

```rust
// ppu_2c02.rs - struct field
open_bus: Cell<u8>,

// write_byte - update open bus on every write
fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
    self.open_bus.set(value);  // Update open bus on every write
    // ... existing match arms
}

// read_byte - update open bus with returned value
fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
    let value = match addr { /* ... */ };
    self.open_bus.set(value);  // Update open bus with returned value
    Ok(value)
}

// Write-only register reads return open bus
fn read_control_register(&self) -> u8 {
    self.open_bus.get()  // $2000 is write-only
}

// Status register combines status bits with open bus
fn read_status_register(&self) -> u8 {
    let status = self.register.borrow().status;
    let open_bus = self.open_bus.get();
    // ... VBlank/latch handling
    (status & 0xE0) | (open_bus & 0x1F)  // Bits 7,6,5 from status; 4-0 from open bus
}
```

---

## Test Results

- All 215 tests pass
- AccuracyCoin "DUMMY WRITE CYCLE" test #1 passes
- Tests #2 and #3 (RMW double write to $2006) also pass (CPU already implements `rmw_overwrite()`)
