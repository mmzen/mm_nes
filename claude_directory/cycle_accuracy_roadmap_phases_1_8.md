# Cycle Accuracy Roadmap - Phases 1-8 (COMPLETED)

**Status**: All 8 phases completed (December 20, 2025)

**Goal**: Transform from instruction-level accurate to true cycle-accurate emulation.

---

## Phase 1: Wire Up Existing DmaController (Foundation)

**Priority**: CRITICAL | **Risk**: Low | **Effort**: Small

**Status**: COMPLETED (Session 11, December 20, 2025)

**Summary**: Wired up the existing `DmaController` to the scheduler. OAM DMA now executes cycle-by-cycle instead of atomically.

**Changes made**:
- `NesConsole`: Added `DmaController` field, `dma_start_page` shared cell
- `PpuDma`: Simplified to signal DMA start via shared cell (no longer does transfer)
- `step_master_cycle()`: Checks for DMA signal, steps DmaController when active
- `DmaController`: Fixed alignment logic (1 idle cycle for even start, 2 for odd)

**Verification**:
- [x] Unit test: OAM DMA takes exactly 513 cycles on even start
- [x] Unit test: OAM DMA takes exactly 514 cycles on odd start
- [x] Unit test: Read/write operations alternate correctly
- [x] Unit test: Reads from correct source addresses
- [x] All 244 tests pass, no regressions

---

## Phase 2: CPU Bus-Cycle Modeling

**Priority**: HIGH | **Risk**: Medium | **Effort**: Large

**Status**: COMPLETED (Session 11, December 20, 2025)

**Problem**: CPU did all memory operations atomically on last cycle. Real 6502 spreads reads/writes across cycles.

**Solution implemented**: Complete refactor of `step_cycle()` to perform exactly one bus operation per cycle.

**Changes made**:
- Added `InstructionState` struct to track intermediate values during execution
- Refactored `CpuCycleState::Executing` to include `opcode`, `cycle`, and `state` fields
- Implemented per-cycle handlers for each addressing mode:
  - `execute_cycle_zero_page()` - 3 cycles read/write, 5 cycles RMW
  - `execute_cycle_zero_page_indexed()` - 4 cycles read/write, 6 cycles RMW
  - `execute_cycle_absolute()` - 4 cycles read/write, 6 cycles RMW
  - `execute_cycle_absolute_indexed()` - 4-5 cycles read, 5 cycles write, 7 cycles RMW
  - `execute_cycle_relative()` - 2-4 cycles for branches
  - `execute_cycle_indirect_jmp()` - 5 cycles with page wrap bug
  - `execute_cycle_indirect_x()` - 6 cycles read/write, 8 cycles RMW
  - `execute_cycle_indirect_y()` - 5-6 cycles read, 6 cycles write, 8 cycles RMW
- Implemented stack operation handlers:
  - `execute_cycle_push_stack()` - PHA/PHP, 3 cycles
  - `execute_cycle_pull_stack()` - PLA/PLP, 4 cycles
  - `execute_cycle_rts()` - RTS, 6 cycles
  - `execute_cycle_rti()` - RTI, 6 cycles
  - `execute_cycle_brk()` - BRK, 7 cycles
  - `execute_cycle_jsr()` - JSR, 6 cycles
- Enhanced `CpuCycleResult` with `BusOperation`, `data`, and `cycle_description` fields
- Updated authorship: `cpu_6502.rs` now Human 40% | Claude 60%

**Key implementation details**:
- Each `step_cycle()` call performs exactly ONE bus operation (read or write)
- Correct page crossing dummy reads for indexed modes
- RMW instructions perform dummy write of original value before writing result
- Stack operations have correct timing (dummy reads, proper SP manipulation)
- Branch instructions correctly handle taken/not-taken and page crossing cases

**Files modified**:
- `cpu_6502.rs` - Major refactor (~1500 lines added)
- `cpu.rs` - Added `BusOperation` enum, enhanced `CpuCycleResult`

**Verification**:
- [x] All 244 tests pass
- [x] All 18 CPU unit tests pass
- [x] Cycle counts match 6502 documentation

---

## Phase 3: NMI/IRQ Polling at Penultimate Cycle

**Priority**: HIGH | **Risk**: Medium | **Effort**: Medium

**Status**: COMPLETED (Session 14, December 20, 2025)

**Problem**: Interrupts were polled after instruction completion. Real 6502 polls during execution.

**Solution implemented**: Latched interrupt polling - NMI/IRQ sampled at START of each CPU cycle, latched values checked after instruction completion.

**Changes made**:
- Added `latched_nmi`, `latched_irq`, `prev_nmi_line_low` fields to `Cpu6502`
- Added `poll_interrupts()` method - samples NMI/IRQ at start of each cycle
- Added `check_and_setup_interrupt_from_latch()` - uses latched values at completion
- Modified `step_cycle()` to poll at start of FetchOpcode and Executing states
- In FetchOpcode: clear latched values, then poll
- In Executing: poll before execution, use latched values at completion

**Key behavior**:
```
Each cycle start: Poll NMI/IRQ, update latched values
Instruction complete: Check latched_nmi/latched_irq
If NMI arrives AFTER final cycle's poll: affects NEXT instruction
```

**Files modified**:
- `cpu_6502.rs` (Human 35% | Claude 65%) - ~70 lines added
- `tests/cpu_6502.rs` (Human 60% | Claude 40%) - 5 new tests

**Verification**:
- [x] Unit test: NMI signaled before final cycle is serviced
- [x] Unit test: NMI signaled after completion delays to next instruction
- [x] Unit test: IRQ respects I flag at instruction completion (SEI test)
- [x] Unit test: NMI has priority over IRQ
- [x] Unit test: Latched state persists through instruction
- [x] All 254 tests pass, no regressions

---

## Phase 4: DMC DMA Mid-Instruction Stealing

**Priority**: HIGH | **Risk**: High | **Effort**: Large

**Status**: COMPLETED (Session 14, December 20, 2025)

**Problem**: DMC DMA must steal 1-4 cycles from CPU mid-instruction, not at boundaries.

**Solution implemented**: The scheduler now checks `apu.needs_dmc_dma()` every cycle. When DMC DMA is needed, it calculates the cycle count based on current CPU/DMA state and initiates the DMC DMA, which delivers the sample to the APU upon completion.

**Changes made**:
- `dma_controller.rs`: Modified `start_dmc_dma()` to accept cycle count parameter
- `dma_controller.rs`: Added `calculate_dmc_dma_cycles()` static method
- `nes_console.rs`: Modified `step_master_cycle()` to:
  - Check `apu.needs_dmc_dma()` every cycle (not just at instruction boundaries)
  - Calculate DMC DMA cycle count based on CPU state
  - Deliver sample to APU via `provide_dmc_sample()` when DMC DMA completes

**DMC DMA cycle calculation**:
- 4 cycles if CPU is writing (cannot interrupt mid-write)
- 3 cycles if CPU is reading
- 2 cycles if OAM DMA is in progress
- 1 cycle if CPU is already halted

**Files modified**:
- `dma_controller.rs` (Human 0% | Claude 100%) - ~40 lines added
- `nes_console.rs` (Human 45% | Claude 55%) - ~30 lines added

**Verification**:
- [x] Unit test: DMC DMA cycle calculation for CPU writing (4 cycles)
- [x] Unit test: DMC DMA cycle calculation for CPU reading (3 cycles)
- [x] Unit test: DMC DMA cycle calculation during OAM DMA (2 cycles)
- [x] Unit test: DMC DMA cycle calculation when CPU halted (1 cycle)
- [x] All 258 tests pass, no regressions

---

## Phase 5: PPU Dot-Level Rendering

**Priority**: MEDIUM | **Risk**: High | **Effort**: Very Large

**Problem**: PPU renders entire scanline atomically. Mid-scanline register changes don't take effect at correct dot.

**Status**: COMPLETED (Session 15-16, December 20, 2025)

### 5.1: Dot-Level State Machine
Replace scanline rendering with per-dot processing:

| Dots | Activity | Status |
|------|----------|--------|
| 0 | Idle | Done |
| 1-256 | Render pixels, shift registers | Done |
| 257 | Copy horizontal bits t->v | Done |
| 258-320 | Sprite fetches | Done |
| 321-336 | First two tiles of next scanline | Done |
| 337-340 | Dummy fetches | Done |

**Changes made**:
- Added `dot_level_rendering: bool` flag to enable per-dot mode (default: true)
- Added `render_dot()` method for per-dot pixel output
- Modified `advance_dots_internal()` to call `render_dot()` for each dot during visible scanlines
- Pixels output using shift registers with fine X scroll

### 5.2: Background Shift Registers
- [x] Two 16-bit shift registers for pattern data (`bg_shift_pattern_lo`, `bg_shift_pattern_hi`)
- [x] Two 16-bit shift registers for attribute data (`bg_shift_attrib_lo`, `bg_shift_attrib_hi`)
- [x] Shift every dot during rendering (`bg_shift_registers()`)
- [x] Load new tile data every 8 dots (`bg_load_shift_registers()`)
- [x] Four 8-bit latches for next tile (`bg_next_tile_id`, `bg_next_tile_attrib`, `bg_next_tile_lo`, `bg_next_tile_hi`)
- [x] Tile fetching methods (`bg_fetch_tile_id()`, `bg_fetch_attribute()`, `bg_fetch_pattern_lo()`, `bg_fetch_pattern_hi()`)
- [x] Fine X scroll for pixel selection (`bg_get_pixel_color()`)

**Unit tests added**:
- `test_shift_registers_initialized_to_zero` - verify initialization
- `test_shift_registers_shift_left` - verify shift operation
- `test_shift_registers_load_tile_data` - verify tile loading
- `test_get_pixel_color_with_fine_x_scroll` - verify fine X scroll
- `test_shift_register_full_tile_cycle` - verify 8-dot tile cycle

### 5.3: Sprite Tile Fetches (Dots 257-320)
- [x] Dot 257: Copy horizontal bits from t to v, reset OAMADDR
- [x] Dots 257-320: Framework for sprite tile fetches (8 sprites x 8 cycles)
- Note: Sprite rendering still atomic (at dot 1) but fetch timing is correct

### 5.4: Mid-Scanline Register Effects
- [x] $2001 (PPUMASK) changes take effect immediately - flags read fresh at each dot
- [x] $2005/$2006 changes affect fetches at correct dot - v and fine_x used directly
- [x] Sprite 0 hit evaluated at exact pixel position - per-dot evaluation with current flag state

### 5.5: Prefetch and Dummy Cycles
- [x] Dots 321-336: Prefetch first two tiles of next scanline
  - Shift registers shift every dot
  - Tile fetch at dot 321, load at dot 328
  - Tile fetch at dot 329, load at dot 336
- [x] Dots 337-340: Dummy nametable fetches (dots 337 and 339)

**Files modified**:
- `ppu_2c02.rs` (Human 60% | Claude 40%) - shift registers, per-dot rendering, alignment fix
- `tests/ppu_2c02.rs` (Human 60% | Claude 40%) - 7 new tests

### Phase 5 Regression Fix: Background Alignment

**Issue**: After Phase 5 implementation, `ppu_sprite_hit/alignment.nes` test showed text cut off on left side.

**Root causes identified**:
1. At dot 1, shift registers were reset to 0 and data loaded into LOWER 8 bits, but `bg_get_pixel_color()` reads from UPPER 8 bits (bit 15-fine_x)
2. Pre-render scanline (261) wasn't doing prefetch at dots 321-336, so scanline 0 started with empty/invalid shift registers

**Fixes applied**:
1. Removed shift register reset at dot 1 - rely on prefetch data from previous scanline
2. Added pre-render scanline prefetch handling for dots 321-336

---

## Phase 6: Odd Frame Skip (NTSC)

**Priority**: MEDIUM | **Risk**: Low | **Effort**: Small

**Status**: COMPLETED (Session 18, December 20, 2025)

**Problem**: NTSC PPU skips one dot on odd frames when rendering enabled. Not implemented.

**Reference**: Pre-render scanline has 340 dots on odd frames (with rendering), 341 on even.

**Solution implemented**:
- Added `frame_odd: bool` field to PPU state
- Toggle frame parity when frame completes
- On pre-render scanline, dot 340, if `frame_odd && rendering_enabled`: skip directly to scanline 0, dot 0

**Changes made**:
- Added `frame_odd` field to `Ppu2c02` struct
- Added frame parity toggle in `advance_dots_internal()` when `frame_ready`
- Added dot skip logic at dot 340 of pre-render scanline

**Files modified**:
- `ppu_2c02.rs` (Human 55% | Claude 45%) - frame parity, skip logic
- `tests/ppu_2c02.rs` - 3 new unit tests

**Verification**:
- [x] Unit test: Odd frame with rendering has 89341 dots
- [x] Unit test: Even frame has 89342 dots
- [x] Unit test: Odd frame without rendering has same dots as even frame (no skip)

---

## Phase 7: CPU/PPU Phase Alignment Verification

**Priority**: LOW | **Risk**: Medium | **Effort**: Medium

**Status**: COMPLETED (Session 18, December 20, 2025)

**Problem**: No verification that CPU cycle 0 aligns with PPU dot 0.

**Solution implemented**:
- Verified existing implementation has perfect alignment (0 drift)
- Added test helpers for `get_total_dots()` and `is_frame_odd()`
- Created comprehensive alignment tests

**Test results**:
- PPU advances exactly 3 dots per CPU cycle (NTSC)
- After 1000 frames: 0 dots drift
- Power-on state verified: scanline 261, dot 0, frame_odd=false

**Files modified**:
- `ppu_2c02.rs` - Added test helpers
- `tests/ppu_2c02.rs` - 3 new alignment tests

**Verification**:
- [x] Unit test: PPU dots = CPU cycles x 3 (exact)
- [x] Unit test: No drift after 1000 frames (0 drift)
- [x] Unit test: Power-on alignment (scanline 261, dot 0)

---

## Phase 8: APU Frame Counter Alignment

**Priority**: LOW | **Risk**: Low | **Effort**: Small

**Status**: COMPLETED (Session 18, December 20, 2025)

**Problem**: APU frame counter may not be aligned with CPU/PPU.

**Solution implemented**:
- Updated `recompute_timing()` to use exact hardware values for NTSC
- 4-step mode: APU cycles [3729, 7457, 11186, 14915] (CPU [7458, 14914, 22372, 29830])
- 5-step mode: APU cycles [3729, 7457, 11186, 14915, 18641] (CPU [7458, 14914, 22372, 29830, 37282])
- PAL/Dendy still use formula calculation (approximate)

**Files modified**:
- `apu_rp2a03.rs` (Human 65% | Claude 35%) - exact timing values, test helpers
- `tests/apu_rp2a03.rs` - 3 new timing tests

**Verification**:
- [x] Unit test: 4-step mode uses exact hardware cycle values
- [x] Unit test: 5-step mode uses exact hardware cycle values
- [x] Unit test: Frame counter steps advance at correct cycles

---

## Dependency Graph

```
Phase 1 (DmaController)
    |
Phase 2 (CPU Bus-Cycle) --> Phase 3 (NMI Timing)
    |                            |
Phase 4 (DMC Mid-Instruction) <--+
    |
Phase 5 (PPU Dot-Level)
    |
Phase 6 (Odd Frame Skip)
    |
Phase 7 (Phase Alignment)
    |
Phase 8 (APU Alignment)
```

---

## Risk Assessment

| Phase | Risk | Mitigation |
|-------|------|------------|
| 1 | Low | Code exists, just needs wiring |
| 2 | Medium | Large refactor, maintain backward compat with `step_instruction()` |
| 3 | Medium | Subtle edge cases, need comprehensive test coverage |
| 4 | High | Architectural change, may affect performance significantly |
| 5 | High | Largest change, may break existing game compatibility |
| 6 | Low | Well-documented, isolated change |
| 7 | Medium | Hard to test without hardware reference |
| 8 | Low | Well-documented timing |

---

## Success Metrics (Target)

| Metric | Before | After |
|--------|--------|-------|
| AccuracyCoin | 74/131 | 120+/131 (target) |
| SingleStep Tests | 100% | 100% |
| Unit Tests | 215 | 274 |
