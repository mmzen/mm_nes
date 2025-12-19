# PPU Open Bus Decay - Archived Task

**Status**: Completed
**Date Completed**: December 19, 2025
**AccuracyCoin Test**: PPU Register Open Bus - FAIL 4 fix

---

## Overview

Implemented PPU open bus decay behavior to pass the AccuracyCoin "PPU Register Open Bus" test FAIL 4: "The PPU data bus value should decay before 1 second passes."

---

## Problem

The PPU had open bus tracking (implemented in a previous session), but the open bus value never decayed. On real NES hardware, the PPU data bus is capacitive and each bit decays independently over time (approximately 600ms).

AccuracyCoin test error codes for PPU Register Open Bus:
1. Reading from a write-only register PPU should return the most recently written value to the PPU data bus.
2. All PPU Registers should update the PPU data bus when written.
3. Bits 0 through 4 when reading from address $2002 should read the PPU data bus.
4. The PPU data bus value should decay before 1 second passes.

---

## PPU Open Bus Decay Behavior

| Aspect | Behavior |
|--------|----------|
| Decay time | ~600ms per bit (3,000,000 PPU dots) |
| Bit independence | Each of 8 bits decays independently |
| Refresh trigger | Writing to any PPU register refreshes all 8 bits |
| $2002 read | Only refreshes bits 7-5 (status bits), bits 4-0 continue to decay |
| Other reads | Refresh all 8 bits |

---

## Solution

1. Added `total_dots: Cell<u64>` field to track total PPU dots since reset
2. Added `open_bus_refresh_dots: [Cell<u64>; 8]` to track when each bit was last refreshed
3. Defined decay constant: `OPEN_BUS_DECAY_DOTS = 3,000,000` (~600ms at NTSC PPU speed)
4. Updated `advance_dots()` to increment `total_dots` on each PPU dot
5. Added `refresh_open_bus_bits(mask, value)` method to refresh specific bits
6. Added `get_decayed_open_bus()` method to compute current value with decay applied
7. Updated `read_byte()` to use decayed value and refresh appropriate bits
8. Updated `write_byte()` to refresh all 8 bits

---

## Files Modified

| File | Change |
|------|--------|
| `ppu_2c02.rs` | Added decay tracking fields, helper methods, updated read/write behavior |
| `tests/ppu_2c02.rs` | Added 6 new unit tests for open bus decay |

---

## Key Code

```rust
// Decay constant - ~600ms worth of PPU dots
const OPEN_BUS_DECAY_DOTS: u64 = 3_000_000;

// New fields in Ppu2c02 struct
total_dots: Cell<u64>,
open_bus_refresh_dots: [Cell<u64>; 8],

// Refresh specific bits
fn refresh_open_bus_bits(&self, mask: u8, value: u8) {
    let current_dots = self.total_dots.get();
    let current_bus = self.open_bus.get();
    let new_bus = (current_bus & !mask) | (value & mask);
    self.open_bus.set(new_bus);

    for bit in 0..8 {
        let bit_mask = 1u8 << bit;
        if (mask & bit_mask) != 0 {
            self.open_bus_refresh_dots[bit].set(current_dots);
        }
    }
}

// Get decayed value
fn get_decayed_open_bus(&self) -> u8 {
    let current_dots = self.total_dots.get();
    let current_bus = self.open_bus.get();
    let mut decayed_bus = 0u8;

    for bit in 0..8 {
        let bit_mask = 1u8 << bit;
        let refresh_time = self.open_bus_refresh_dots[bit].get();
        let elapsed = current_dots.saturating_sub(refresh_time);
        if elapsed < OPEN_BUS_DECAY_DOTS {
            decayed_bus |= current_bus & bit_mask;
        }
    }
    decayed_bus
}

// read_byte - refresh based on register
match addr {
    0x02 => self.refresh_open_bus_bits(0xE0, value), // $2002: only bits 7-5
    _ => self.refresh_open_bus_bits(0xFF, value),     // All other: all 8 bits
}

// write_byte - always refresh all 8 bits
self.refresh_open_bus_bits(0xFF, value);
```

---

## New Unit Tests

1. `test_open_bus_set_on_write` - Writing sets open bus value
2. `test_write_only_registers_return_open_bus` - Write-only registers return open bus
3. `test_status_register_combines_status_and_open_bus` - $2002 combines status + open bus
4. `test_open_bus_decays_over_time` - Value decays to 0 after ~600ms
5. `test_writing_refreshes_open_bus` - Writing refreshes and prevents decay
6. `test_status_read_only_refreshes_upper_bits` - $2002 read only refreshes bits 7-5

---

## Test Results

- All 221 tests pass
- AccuracyCoin "PPU Register Open Bus" test passes (FAIL 4 fixed)
