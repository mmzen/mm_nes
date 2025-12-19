# Cycle-Stepping Infrastructure - Completed Task

**Status**: Completed
**Date Completed**: December 19, 2025
**Session**: 9

---

## Overview

**Goal**: Transform the emulator from instruction-level to cycle-level synchronization between CPU, PPU, and APU.

**Problem Statement**:
The architecture synchronized components at instruction boundaries:
- CPU executes complete instruction → catch-up PPU/APU → repeat
- This causes timing issues: DMC DMA can't steal cycles mid-instruction, PPU register changes don't apply at correct dot

**Solution**: Master Clock Architecture - advance all components one CPU cycle at a time:
- CPU: 1 cycle (or halted if DMA active)
- PPU: 3 dots per CPU cycle
- APU: 1 tick per CPU cycle

---

## Implementation Phases

### Phase 1: CPU Cycle-Stepping State Machine
**Files**: `cpu.rs`, `cpu_6502.rs`

**Changes**:
- Added `CpuCycleState` enum with `FetchOpcode`, `Executing`, and `Halted` states
- Added `step_cycle()` method that advances CPU by one cycle
- Added `is_mid_instruction()`, `is_halted()`, `halt_cycles()`, `get_cycles()` methods
- Added `CpuCycleResult` struct to report cycle activity
- Backward compatibility maintained with existing `step_instruction()` wrapper

**Key code**:
```rust
enum CpuCycleState {
    FetchOpcode,
    Executing {
        instruction: &'static Instruction,
        operand: Operand,
        total_cycles: u32,
        current_cycle: u32,
        is_interrupt: bool,
    },
    Halted { cycles_remaining: u32 },
}

pub struct CpuCycleResult {
    pub instruction_complete: bool,
    pub memory_read: bool,
    pub memory_write: bool,
    pub address: Option<u16>,
    pub halted: bool,
}
```

**Tests added**: 5 new unit tests for cycle-stepping functionality

---

### Phase 2: DMA Controller
**Files**: `dma_controller.rs` (new), `lib.rs`

**Changes**:
- Created `DmaController` struct managing OAM and DMC DMA
- `OamDmaState`: Tracks OAM DMA state (page, byte index, read/write phase, cycle count)
- `DmcDmaState`: Tracks DMC DMA state (address, cycles remaining)
- `step_cycle()`: Executes one cycle of DMA (read/write alternating for OAM)
- `start_oam_dma(page)`: Starts OAM DMA from specified page (513-514 cycles)
- `start_dmc_dma(address)`: Starts DMC DMA fetch (1-4 cycles)
- Proper odd/even cycle alignment for OAM DMA

**Key behavior**:
- OAM DMA takes 513 cycles on even CPU cycle, 514 on odd
- First cycle is always a dummy read (alignment)
- Alternates between read from source and write to OAM

**Tests added**: 6 new unit tests for DMA controller

---

### Phase 3: Master Scheduler in NesConsole
**Files**: `nes_console.rs`, `ppu_dma.rs`

**Changes**:
- Added `step_master_cycle()` method - advances all components by one CPU cycle
- Added `step_frame_cycle_accurate()` method for cycle-accurate frame rendering
- PpuDma signals DMA halt cycles via shared `Rc<Cell<u32>>`
- PpuDma tracks CPU cycle parity via shared `Rc<Cell<bool>>` for 513/514 cycle alignment
- `new_with_cycle_tracking()` constructor for PpuDma with shared cells
- NesConsoleBuilder wires up shared cells between PpuDma and NesConsole

**Key code**:
```rust
pub fn step_master_cycle(&mut self) -> Result<(CpuCycleResult, Option<NesFrame>, Option<NesSamples>), NesConsoleError> {
    // Check if OAM DMA was triggered and needs to halt CPU
    let dma_cycles = self.dma_halt_cycles.get();
    if dma_cycles > 0 {
        self.dma_halt_cycles.set(0);
        self.cpu.borrow_mut().halt_cycles(dma_cycles);
    }
    // Execute one CPU cycle
    let cpu_result = self.cpu.borrow_mut().step_cycle()?;
    // Advance PPU by 3 dots
    let ppu_frame = self.ppu.borrow_mut().advance_dots(3)?;
    // Advance APU by 1 tick
    // ...
}
```

**Backward compatibility**: Existing `step_instruction()` and `step_frame()` still work

---

### Phase 4: PPU Integration Refinements
**Files**: `ppu.rs`, `ppu_2c02.rs`

**Changes**:
- Added `advance_dots()` method to PPU trait for direct dot-level control
- Added `get_dot()` and `get_scanline()` methods to PPU trait for state inspection
- Renamed internal `advance_dots` to `advance_dots_internal` to avoid conflict with trait method
- Master scheduler now uses `advance_dots(3)` directly instead of `run()` for precise control

---

### Phase 5: APU Cycle-Stepping
**Files**: `apu.rs`, `apu_rp2a03.rs`

**Changes**:
- Added `step_cycle()` method to APU trait - advances APU by exactly one CPU cycle
- Added `needs_dmc_dma()` method - returns address if DMC needs sample fetch
- Added `provide_dmc_sample(value)` method - provides fetched sample to DMC
- APU clocks DMC and triangle at CPU rate
- APU clocks pulses, noise, and frame sequencer at half CPU rate (every other cycle)
- Sample accumulation handled via sound player buffer

**Key code**:
```rust
fn step_cycle(&mut self) -> Result<Option<f32>, ApuError> {
    // DMC and triangle channels are clocked at the CPU clock rate
    self.clock_dmc_timer()?;
    self.clock_triangle_timer();

    // Other channels and frame counter are clocked at half CPU rate
    if !self.cpu_cycle_odd {
        self.clock_pulse_timers();
        self.clock_noise_timer();
        self.clock_frame_sequencer(1)?;
        // Sample generation...
    }
    self.cpu_cycle_odd = !self.cpu_cycle_odd;
    Ok(None)
}

fn needs_dmc_dma(&self) -> Option<u16> {
    if self.dmc.enabled && self.dmc.bytes_remaining > 0 && self.dmc.sample_buffer.is_none() {
        self.dmc.current_address
    } else {
        None
    }
}
```

---

## Outcome

- **All 5 phases complete**
- **Tests**: 237/237 passing (11 new tests added)
- **Backward compatibility**: `step_frame()` and `step_instruction()` still work
- **New capability**: `step_frame_cycle_accurate()` for true cycle-accurate emulation
- **Foundation laid** for fixing DEF-002 (DMC DMA timing) and other timing-sensitive issues

---

## Files Modified

- `cpu.rs` - Added `CpuCycleResult` struct and new trait methods
- `cpu_6502.rs` - Added `CpuCycleState` enum and `step_cycle()` implementation
- `tests/cpu_6502.rs` - Added 5 unit tests for cycle-stepping
- `dma_controller.rs` - New file (~270 lines)
- `lib.rs` - Added `dma_controller` module
- `ppu_dma.rs` - Added shared cells for DMA signaling
- `nes_console.rs` - Added `step_master_cycle()` and `step_frame_cycle_accurate()`
- `ppu.rs` - Added `advance_dots()`, `get_dot()`, `get_scanline()` to trait
- `ppu_2c02.rs` - Implemented new PPU trait methods
- `apu.rs` - Added `step_cycle()`, `needs_dmc_dma()`, `provide_dmc_sample()` to trait
- `apu_rp2a03.rs` - Implemented new APU trait methods

---

## Future Work

- Integrate DMA controller with master scheduler for proper DMC DMA cycle stealing mid-instruction
- This would fix DEF-002 (DMC DMA timing) and pass AccuracyCoin "DMA + OPEN BUS" FAIL 2
