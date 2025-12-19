# APU Open Bus Implementation

**Date**: December 19, 2025
**Status**: Completed
**AccuracyCoin Test**: DMA + OPEN BUS - FAIL 1 passes

---

## Problem

AccuracyCoin "DMA + OPEN BUS" test FAIL 1: "LDA $4000 should not read back $00 if a DMA did not occur."

The APU was returning 0 for write-only registers ($4000-$4013, $4017) instead of the open bus value (the last value on the data bus).

---

## Solution

Extended the shared data bus mechanism to the APU:

1. **NESBus** already tracks data bus value via `Rc<Cell<u8>>`
2. Added `get_data_bus()` method to NESBus to expose the shared reference
3. **APU** now receives the shared data bus at construction
4. APU's `read_byte()` returns `data_bus.get()` for write-only registers

---

## Files Modified

| File | Changes |
|------|---------|
| `mmnes_core/src/nes_bus.rs` | Added `get_data_bus()` method |
| `mmnes_core/src/apu_rp2a03.rs` | Added `data_bus` field, `read_open_bus()` method, updated constructor |
| `mmnes_core/src/nes_console.rs` | Updated builder to pass data_bus to APU |
| `mmnes_core/src/tests/apu_rp2a03.rs` | Updated test helper to pass data_bus |

---

## Code Details

### APU struct addition:
```rust
pub struct ApuRp2A03<T: SoundPlayback, U: CPU + ?Sized, V: Bus + ?Sized> {
    // ... existing fields ...
    data_bus: Rc<Cell<u8>>,  // Shared data bus for open bus behavior
}
```

### APU read_byte:
```rust
fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
    let value = match addr {
        0x15 => self.read_channels_status()?,  // Only readable register
        _ => self.read_open_bus(),  // Write-only registers return open bus
    };
    Ok(value)
}
```

---

## Test Results

- All 215+ unit tests pass
- AccuracyCoin "DMA + OPEN BUS" FAIL 1 passes

---

## Notes

FAIL 2 ("DMC DMA was either on the wrong cycle, or it did not update the data bus") remains unresolved. This requires cycle-accurate DMC DMA timing which is challenging with instruction-level emulation. An attempted fix (pre-instruction DMC check) was rolled back as it didn't solve the issue.
