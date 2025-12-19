# ROM IS NOT WRITABLE - Archived Task

**Status**: Completed
**Date Completed**: December 19, 2025
**AccuracyCoin Test**: PASS

---

## Overview

Implemented read-only protection for ROM memory banks to pass the AccuracyCoin "ROM IS NOT WRITABLE" test.

---

## Problem

The NROM cartridge's `write_byte` method forwarded writes to PRG-ROM memory via `MemoryBank`, which actually modified the ROM data. On real NES hardware, writes to ROM are silently ignored.

---

## Solution

Added read-only support to `MemoryBank`:

1. Added `is_read_only: bool` field to `MemoryBank` struct
2. Added `set_read_only()` method to mark banks as read-only after initialization
3. Updated `write_byte()` to return `Ok(())` without modifying memory when `is_read_only` is true
4. Updated `create_split_rom_memory()` in `cartridge.rs` to call `set_read_only()` after loading ROM data
5. Fixed NROM cartridge `write_byte/write_word` to explicitly return `Ok(())` (NROM has no mapper control)

---

## Files Modified

| File | Change |
|------|--------|
| `memory_bank.rs` | Added `is_read_only` field and `set_read_only()` method |
| `cartridge.rs` | Updated `create_split_rom_memory()` to mark ROM banks as read-only |
| `nrom_cartridge.rs` | `write_byte/write_word` now return `Ok(())` without modifying ROM |
| `tests/memory_bank.rs` | Added 2 unit tests for read-only behavior |

---

## Key Code

```rust
// memory_bank.rs
fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
    if self.is_read_only {
        return Ok(()); // Silently ignore writes to ROM
    }
    // ... existing write logic
}

pub fn set_read_only(&mut self) {
    self.is_read_only = true;
}
```

```rust
// nrom_cartridge.rs
fn write_byte(&mut self, _addr: u16, _value: u8) -> Result<(), MemoryError> {
    // NROM has no mapper control - writes to ROM space are silently ignored
    Ok(())
}
```

---

## Test Results

- All 215 tests pass
- AccuracyCoin "ROM IS NOT WRITABLE" test passes
