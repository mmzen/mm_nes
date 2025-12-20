# PLAN.md - Development Tracker

This file tracks the development progress of mmnes, documenting what has been done, what is planned, and serving as a continuity document across sessions.

---

## Current Status

**Last updated**: December 20, 2025

The emulator is functional with **cycle-accurate interrupt polling** (Phase 3 complete). Phases 1-3 of cycle accuracy roadmap done.

**Honest characterization**:
- CPU: **Cycle-accurate bus operations** with **interrupt polling on each cycle**
- PPU: Scanline-accurate (correct frame structure, atomic scanline rendering)
- DMA: **Cycle-by-cycle OAM DMA** (513/514 cycles with proper alignment)
- Scheduler: Cycle-synchronized (correct 1:3 ratio, cycle-level stepping)
- Interrupts: **Latched polling** - NMI/IRQ sampled at start of each cycle, used at instruction completion

**Current focus**: True cycle accuracy roadmap - Phases 1-3 complete, Phase 4 (DMC DMA mid-instruction stealing) next.

**AccuracyCoin Score**: 90/131 (target: 120+/131 after roadmap completion)

---

## Completed Work

### Human Contributions (Pre-December 2025)
- 6502 CPU implementation (all official + unofficial opcodes)
- PPU 2C02 (background, sprites, sprite 0 hit, VBL/NMI)
- APU RP2A03 (pulses, triangle, noise, DMC)
- Mappers: NROM, UxROM, MMC1, MMC2
- iNES ROM loader
- GUI with eframe/egui
- CPU debugger/disassembler
- Sound playback via SDL2
- Initial LLM integration (OpenAI, hint overlay)
- AccuracyCoin score: 74/131 (now 90/131 after Claude contributions)

### Claude Contributions (December 2025 → Present)

| Task | Outcome | Details |
|------|---------|---------|
| SingleStepTests framework | CPU passes 100% (2,560,000/2,560,000) | `mmnes_core/src/tests/singlestep/` |
| README.md update | Reflects human-to-AI experiment framing | - |
| Cycle-accurate timing refactoring | AccuracyCoin 74→82→90/131, audio synced | [Full details](claude_directory/cycle_accurate_timing_refactoring.md) |
| ROM IS NOT WRITABLE fix | AccuracyCoin test passes | [Full details](claude_directory/rom_is_not_writable.md) |
| PPU Open Bus | AccuracyCoin DUMMY WRITE CYCLE passes | [Full details](claude_directory/ppu_open_bus.md) |
| CPU/Bus Open Bus | AccuracyCoin OPEN BUS FAIL 1 passes | Data bus tracking in NESBus |
| APU Open Bus | AccuracyCoin DMA + OPEN BUS FAIL 1 passes | [Full details](claude_directory/apu_open_bus.md) |
| PPU Open Bus Decay | AccuracyCoin "PPU Register Open Bus" passes | [Full details](claude_directory/ppu_open_bus_decay.md) |
| PPU Read Buffer | AccuracyCoin "PPU READ BUFFER" passes | Palette read now updates buffer with nametable data |
| PPU Palette 6-bit | AccuracyCoin "PALETTE RAM QUIRKS" FAIL 5 passes | Palette reads return 6-bit value + open bus upper 2 bits |
| Cycle-stepping infrastructure | All 5 phases complete, **activated in frontend** | [Full details](claude_directory/cycle_stepping_infrastructure.md) |
| Phase 1: DmaController wiring | OAM DMA now cycle-by-cycle (513/514 cycles) | Signal-based DMA, proper alignment |
| Phase 2: CPU Bus-Cycle Modeling | **step_cycle() performs one bus op per cycle** | All addressing modes, stack ops, ~1500 lines |
| Phase 3: NMI/IRQ Polling | **Latched interrupt polling at cycle start** | Penultimate cycle timing, 5 new tests |

---

## In Progress

*No tasks currently in progress - Phase 3 complete.*

## Planned Work: True Cycle Accuracy Roadmap

**Goal**: Transform from instruction-level accurate to true cycle-accurate emulation.

**Current State**: Instruction-accurate CPU, scanline-accurate PPU, boundary-synchronized components.

**Target State**: Per-cycle bus modeling, dot-level PPU, sub-instruction interrupt polling, cycle-by-cycle DMA.

---

### Phase 1: Wire Up Existing DmaController (Foundation) ✓ COMPLETED

**Priority**: CRITICAL | **Risk**: Low | **Effort**: Small

**Status**: ✓ **COMPLETED** (Session 11, December 20, 2025)

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

**Acceptance criteria**: ✓ OAM DMA executes cycle-by-cycle through scheduler.

---

### Phase 2: CPU Bus-Cycle Modeling ✓ COMPLETED

**Priority**: HIGH | **Risk**: Medium | **Effort**: Large

**Status**: ✓ **COMPLETED** (Session 11, December 20, 2025)

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

**Acceptance criteria**: ✓ Each CPU cycle maps to exactly one bus operation.

---

### Phase 3: NMI/IRQ Polling at Penultimate Cycle ✓ COMPLETED

**Priority**: HIGH | **Risk**: Medium | **Effort**: Medium

**Status**: ✓ **COMPLETED** (Session 14, December 20, 2025)

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
- [ ] blargg `vbl_nmi_timing/` tests (requires ROM testing)

**Acceptance criteria**: ✓ Interrupts polled each cycle, latched values used at completion.

---

### Phase 4: DMC DMA Mid-Instruction Stealing

**Priority**: HIGH | **Risk**: High | **Effort**: Large

**Problem**: DMC DMA must steal 1-4 cycles from CPU mid-instruction, not at boundaries.

**Reference**: http://wiki.nesdev.org/w/index.php/APU_DMC

**Tasks**:
1. APU signals DMC DMA need via `needs_dmc_dma()` (exists)
2. Scheduler checks DMC need EVERY cycle (not just at instruction boundary)
3. When DMC DMA triggered mid-instruction:
   - Pause CPU cycle state (preserve micro-op position)
   - Execute 1-4 DMA cycles
   - Resume CPU from paused state
4. DMC DMA cycle count depends on CPU state:
   - 4 cycles if CPU is writing
   - 3 cycles if CPU is reading
   - 2 cycles if during OAM DMA
   - 1 cycle if halted

**Files to modify**:
- `nes_console.rs` - Check DMC every cycle in scheduler
- `dma_controller.rs` - Add DMC DMA state machine
- `apu_rp2a03.rs` - Proper DMC sample request timing
- `cpu_6502.rs` - Expose current bus activity for DMC timing

**Verification**:
- [ ] AccuracyCoin "DMA + OPEN BUS" FAIL 2 passes
- [ ] blargg `dmc_dma_during_read4.nes` passes
- [ ] Unit test: DMC DMA during LDA steals 3 cycles

**Acceptance criteria**: DMC DMA can interrupt CPU mid-instruction.

---

### Phase 5: PPU Dot-Level Rendering

**Priority**: MEDIUM | **Risk**: High | **Effort**: Very Large

**Problem**: PPU renders entire scanline atomically. Mid-scanline register changes don't take effect at correct dot.

**Tasks**:

#### 5.1: Dot-Level State Machine
Replace scanline rendering with per-dot processing:

| Dots | Activity |
|------|----------|
| 0 | Idle |
| 1-256 | Render pixels, shift registers |
| 257 | Copy horizontal bits t→v |
| 258-320 | Sprite fetches |
| 321-336 | First two tiles of next scanline |
| 337-340 | Dummy fetches |

#### 5.2: Background Shift Registers
- Two 16-bit shift registers for pattern data
- Two 8-bit latches for attribute data
- Shift every dot during rendering
- Load new tile data every 8 dots

#### 5.3: Sprite Evaluation Per-Dot
- Dots 1-64: Clear secondary OAM (1 byte/dot)
- Dots 65-256: Evaluate sprites (varies)
- Dots 257-320: Fetch sprite patterns

#### 5.4: Mid-Scanline Register Effects
- $2001 (PPUMASK) changes take effect immediately
- $2005/$2006 changes affect fetches at correct dot
- Sprite 0 hit evaluated at exact pixel position

**Files to modify**:
- `ppu_2c02.rs` - Major rewrite of rendering pipeline
- `ppu.rs` - Update trait if needed

**Verification**:
- [ ] AccuracyCoin "RENDERING FLAG BEHAVIOR" passes
- [ ] blargg `sprite_hit_tests/` all pass
- [ ] Unit test: Mid-scanline scroll change affects correct pixels
- [ ] Unit test: Disabling rendering mid-scanline stops at correct dot

**Acceptance criteria**: Register changes take effect at exact dot.

---

### Phase 6: Odd Frame Skip (NTSC)

**Priority**: MEDIUM | **Risk**: Low | **Effort**: Small

**Problem**: NTSC PPU skips one dot on odd frames when rendering enabled. Not implemented.

**Reference**: Pre-render scanline has 340 dots on odd frames (with rendering), 341 on even.

**Tasks**:
1. Add `frame_odd: bool` to PPU state
2. Toggle on each frame completion
3. On pre-render scanline, dot 339:
   - If `frame_odd && rendering_enabled`: skip to dot 0 of scanline 0
   - Otherwise: continue to dot 340, then dot 0

**Files to modify**:
- `ppu_2c02.rs` - Add frame parity, skip logic

**Verification**:
- [ ] blargg `10-even_odd_frames.nes` passes
- [ ] Unit test: Odd frame with rendering has 89341 dots
- [ ] Unit test: Even frame has 89342 dots

**Acceptance criteria**: Frame timing matches hardware exactly.

---

### Phase 7: CPU/PPU Phase Alignment Verification

**Priority**: LOW | **Risk**: Medium | **Effort**: Medium

**Problem**: No verification that CPU cycle 0 aligns with PPU dot 0.

**Tasks**:
1. Define alignment point: CPU cycle after reset aligns with PPU dot
2. Add internal consistency checks
3. Create long-running drift test
4. Verify alignment doesn't drift over 10,000 frames

**Verification**:
- [ ] Unit test: After 10,000 frames, (ppu_dots % 3) == (cpu_cycles % 1)
- [ ] Unit test: Power-on alignment matches hardware

**Acceptance criteria**: No timing drift over extended operation.

---

### Phase 8: APU Frame Counter Alignment

**Priority**: LOW | **Risk**: Low | **Effort**: Small

**Problem**: APU frame counter may not be aligned with CPU/PPU.

**Tasks**:
1. Verify frame counter clocks at correct CPU cycles
2. Ensure 4-step mode: cycles 7457, 14913, 22371, 29828/29829
3. Ensure 5-step mode: cycles 7457, 14913, 22371, 29829, 37281

**Files to modify**:
- `apu_rp2a03.rs` - Verify/fix frame counter timing

**Verification**:
- [ ] blargg `apu_test/4-jitter.nes` passes
- [ ] Unit test: Frame counter IRQ fires at exact cycle

---

### Dependency Graph

```
Phase 1 (DmaController)
    ↓
Phase 2 (CPU Bus-Cycle) ──→ Phase 3 (NMI Timing)
    ↓                            ↓
Phase 4 (DMC Mid-Instruction) ←──┘
    ↓
Phase 5 (PPU Dot-Level)
    ↓
Phase 6 (Odd Frame Skip)
    ↓
Phase 7 (Phase Alignment)
    ↓
Phase 8 (APU Alignment)
```

---

### Risk Assessment

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

### Success Metrics

| Metric | Current | Target |
|--------|---------|--------|
| AccuracyCoin | 90/131 | 120+/131 |
| blargg NMI timing | Unknown | 12/12 |
| blargg sprite hit | Unknown | All pass |
| blargg CPU timing | Pass | Pass |
| blargg APU tests | Unknown | All pass |

---

### Estimated Effort

| Phase | Complexity | Estimated Work |
|-------|------------|----------------|
| 1 | Simple | 1-2 sessions |
| 2 | Complex | 3-5 sessions |
| 3 | Medium | 2-3 sessions |
| 4 | Complex | 3-4 sessions |
| 5 | Very Complex | 5-8 sessions |
| 6 | Simple | 1 session |
| 7 | Medium | 1-2 sessions |
| 8 | Simple | 1 session |

**Total**: ~17-26 sessions for full cycle accuracy

---

## Known Defects

| ID | Description | Severity | Status |
|----|-------------|----------|--------|
| DEF-001 | Audio crackles/pops during playback | Low | Open |
| DEF-002 | DMC DMA timing not cycle-accurate | Medium | Open |
| DEF-003 | Rendering Flag Behavior FAIL 2 | Medium | Deferred |

### DEF-001: Audio Crackles
**Description**: Occasional audio crackles/pops during emulation playback.
**Possible causes**:
- Sample buffer underruns in the audio pipeline
- Timing jitter from fine-grained synchronization
- SDL2 audio callback timing issues
**Notes**: Audio is synchronized with video; only quality is affected.

### DEF-002: DMC DMA Timing
**Description**: AccuracyCoin "DMA + OPEN BUS" FAIL 2 - DMC DMA occurs on wrong cycle or doesn't update data bus correctly.
**Root cause**: DMC DMA currently occurs during APU catch-up (after CPU instruction execution), but real hardware performs DMA mid-instruction at specific cycles.
**Attempted fixes**:
- Pre-instruction DMA check (rolled back - didn't solve issue)
**Required**: Cycle-accurate emulation where DMC DMA can interrupt CPU mid-instruction.
**Notes**: This is a fundamental architecture limitation of instruction-level emulation.

### DEF-004: Arbitrary Sprite Zero
**Description**: AccuracyCoin "ARBITRARY SPRITE ZERO" test fails.
**Test behavior**: The test checks two conditions:
1. FAIL 1: Only sprite at OAM index 0 should trigger sprite zero hit (when OAMADDR=0)
2. FAIL 2: When OAMADDR is set mid-frame, the sprite at OAMADDR/4 should be treated as "sprite zero"
**Attempted fixes**:
1. Mark first in-range sprite as sprite0 → caused FAIL 1 regression
2. Mark sprite at OAMADDR/4 as sprite0 → passes FAIL 1, still fails FAIL 2
**Root cause hypothesis**: The test requires precise timing coordination between OAMADDR writes and sprite evaluation. Current implementation reads OAMADDR at sprite evaluation time, but the test may require OAMADDR to affect sprite zero determination at a specific PPU cycle (cycle 66) during evaluation.
**Notes**: Deferred pending further investigation of PPU sprite evaluation timing.

### DEF-003: Rendering Flag Behavior FAIL 2
**Description**: AccuracyCoin "RENDERING FLAG BEHAVIOR" test FAIL 2 - "Background shift registers should be initialized and clocked when only rendering sprites."
**Test behavior**: Test enables only sprites ($10) during h-blank, expects background shift registers to be populated, then enables both flags mid-scanline before sprite 0 hit position.
**Attempted fixes**:
1. Background fetches when only sprites enabled (`render_background()` called when ShowSprites=true) - implemented, retained
2. Sprite 0 hit detection deferred to hit dot with mask flag check - implemented, retained
3. Checked various sprite 0 hit conditions (rendering_enabled OR vs AND) - tested both approaches
**Root cause hypothesis**: The test requires dot-level accuracy for mid-scanline mask changes. Current implementation renders entire scanline at dot 1 and may not correctly handle the precise timing of when shift registers are populated vs when sprite 0 hit is evaluated.
**Notes**: Deferred pending further investigation. Current fixes are retained as they represent correct hardware behavior even if timing precision is insufficient for this specific test.

---

## Session Log

### Session: December 20, 2025 (session 14 - Phase 3 NMI/IRQ polling)
- **Implemented Phase 3: NMI/IRQ Polling at Penultimate Cycle**
- **Problem solved**: Interrupts were polled AFTER instruction completion. Real 6502 polls during execution, and if NMI arrives on the last cycle (after polling), it affects the NEXT instruction.
- **Solution**: Latched interrupt polling
  - Added `latched_nmi`, `latched_irq`, `prev_nmi_line_low` fields to `Cpu6502`
  - Added `poll_interrupts()` - samples NMI/IRQ state at start of each CPU cycle
  - Added `check_and_setup_interrupt_from_latch()` - uses latched values at instruction completion
  - Modified `step_cycle()` to call `poll_interrupts()` at start of FetchOpcode and Executing states
  - In FetchOpcode: clear latched values first, then poll
  - In Executing: poll before executing, use latched values when instruction completes
  - IRQ re-checks I flag at completion (SEI can prevent IRQ service even if latched)
- **Files modified**:
  - `cpu_6502.rs` (Human 35% | Claude 65%) - ~70 lines added
  - `tests/cpu_6502.rs` (Human 60% | Claude 40%) - 5 new tests
- **New unit tests**:
  - `nmi_signaled_before_final_cycle_is_serviced`
  - `nmi_signaled_after_instruction_completion_delays_to_next_instruction`
  - `irq_respects_i_flag_at_instruction_completion`
  - `nmi_has_priority_over_irq`
  - `nmi_latched_state_persists_even_if_cleared_before_completion`
- **Test results**: All 254 tests pass (249 → 254, +5 new tests)
- **Status**: Phase 3 complete, Phase 4 (DMC DMA mid-instruction stealing) next

### Session: December 20, 2025 (session 13 - cycle-accurate timing fixes)
- **Continued debugging cycle-accurate mode** - test ROMs pass but show misaligned/missing tiles, SMB shows blank screen
- **Fixed interrupt cycle accounting bug**: When NMI/IRQ fires after instruction completion, the 7 interrupt cycles were being added to the CPU's internal counter, but the PPU/APU weren't being advanced by those cycles. This caused PPU desync.
  - Added `interrupt_cycles` field to `CpuCycleResult` in `cpu.rs`
  - Updated `step_cycle()` in `cpu_6502.rs` to set `result.interrupt_cycles` when interrupt fires
  - Updated `step_master_cycle()` in `nes_console.rs` to advance PPU/APU by `(1 + interrupt_cycles) * 3` dots
- **Fixed CPU cycle parity tracking bug**: DMA alignment depends on odd/even CPU cycle. When interrupt fired (consuming 8 total cycles = 1 + 7), parity was toggled only once instead of accounting for all 8 cycles.
  - Changed parity logic to only toggle when `total_cpu_cycles % 2 == 1` (odd number of cycles)
- **Synchronized PPU counter initialization**: Changed `ppu_counter` to start at `CYCLE_START_SEQUENCE` (7) instead of 0, matching `cpu_counter`
- **Files modified**:
  - `cpu.rs` (Human 75% | Claude 25%) - added `interrupt_cycles` field
  - `cpu_6502.rs` (Human 40% | Claude 60%) - set interrupt_cycles in step_cycle
  - `nes_console.rs` (Human 55% | Claude 45%) - updated step_master_cycle timing
- **Test results**: All 249 tests pass, 11 DMA controller tests pass
- **Status**: Core timing fixes applied, user testing required to verify ROM behavior

### Session: December 20, 2025 (session 12 - bug fix)
- **Fixed critical `is_pc_dirty` bug** that caused blank screen in cycle-accurate mode
- **Root cause**: PC-modifying instructions (branches, JMP indirect, JSR, RTS, RTI, BRK) called `set_pc()` which sets `is_pc_dirty=true`, but didn't call `finalize_instruction()`. This left `is_pc_dirty=true` for the NEXT instruction, causing `finalize_instruction()` to skip PC advancement, resulting in infinite loops.
- **Fix**: After calling `set_pc()` in these instructions, immediately clear `is_pc_dirty=false` to ensure the next instruction advances PC correctly.
- **Files modified**: `cpu_6502.rs` (Human 40% | Claude 60%) - 7 locations fixed:
  - `execute_cycle_relative()` cycles 2 and 3 (branches)
  - `execute_cycle_indirect_jmp()` cycle 4
  - `execute_cycle_rts()` cycle 5
  - `execute_cycle_rti()` cycle 5
  - `execute_cycle_brk()` cycle 6
  - `execute_cycle_jsr()` cycle 5
- **Added regression test**: `step_cycle_executes_branch_taken_and_continues()` - verifies PC advancement after branches
- **Test results**: All 249 tests pass (248 → 249, +1 new test)

### Session: December 20, 2025 (session 11)
- **Conducted cycle accuracy audit** - comprehensive analysis of "cycle accurate" and "cycle precise scheduler" claims
- **Key findings**:
  1. CPU `step_cycle()` does all memory ops on last cycle (instruction-level, not cycle-level)
  2. OAM DMA is atomic despite `DmaController` existing (dead code)
  3. DMC DMA cannot steal mid-instruction (acknowledged limitation)
  4. NMI polled after instruction completion (should be penultimate cycle)
  5. PPU renders scanlines atomically (not dot-level)
  6. Odd frame skip NOT implemented
- **Created 8-phase roadmap** for true cycle accuracy:
  - Phase 1: Wire up existing DmaController (Low risk, quick win)
  - Phase 2: CPU bus-cycle modeling (Major refactor)
  - Phase 3: NMI/IRQ penultimate cycle polling
  - Phase 4: DMC DMA mid-instruction stealing
  - Phase 5: PPU dot-level rendering (Largest change)
  - Phase 6: Odd frame skip
  - Phase 7: CPU/PPU phase alignment verification
  - Phase 8: APU frame counter alignment
- **Completed Phase 1: Wire Up DmaController**
  - Added `DmaController` to `NesConsole` struct
  - Replaced atomic DMA in `PpuDma` with signal-based approach
  - Wired `DmaController.step_cycle()` into `step_master_cycle()` scheduler
  - Fixed OAM DMA alignment logic: 1 idle cycle (even start), 2 idle cycles (odd start)
  - Added 6 new unit tests verifying cycle-by-cycle DMA behavior
- **Files modified**: `nes_console.rs`, `ppu_dma.rs`, `dma_controller.rs`, `tests/ppu_dma.rs`
- **Test results**: All 244 tests pass (6 new DMA controller tests added)
- **Authorship updates**: `nes_console.rs` Human 60%/Claude 40%, `ppu_dma.rs` Human 40%/Claude 60%

**Phase 2 progress**:
- Enhanced `CpuCycleResult` with `BusOperation`, `data`, `cycle_description` fields
- Created `cpu_cycle_model.rs` design document for micro-operations
- Updated `step_cycle()` to populate bus activity information
- Added `BusOperation` enum to `cpu.rs`

**Files modified this session**:
- `nes_console.rs` (Human 60%/Claude 40%)
- `ppu_dma.rs` (Human 40%/Claude 60%)
- `dma_controller.rs` (Human 0%/Claude 100%)
- `cpu.rs` (Human 80%/Claude 20%)
- `cpu_6502.rs` (Human 60%/Claude 40%)
- `tests/ppu_dma.rs` (Human 30%/Claude 70%)
- `cpu_cycle_model.rs` (Human 0%/Claude 100%) - NEW

**Test results**: All 244 tests pass

**Contribution ratio**: Human 41% | Claude 59% (cumulative for this session)

### Session: December 20, 2025 (session 10)
- **Fixed CPU step_cycle() cycle counting bug**: Instructions were taking N+1 cycles instead of N (e.g., 2-cycle instruction took 3 calls). Fixed by changing check from `current_cycle >= total_cycles` to `current_cycle + 1 >= total_cycles`.
- **Fixed CPU step_cycle() interrupt handling bug**: When NMI/IRQ occurred, `cycle_state` wasn't being reset to `FetchOpcode`, causing CPU to get stuck. Now always transitions to `FetchOpcode` after instruction completion.
- **Added command line argument** `-i` / `--instruction-level` to switch between cycle-accurate (default) and instruction-level modes
- Debug mode (step instruction, debug run) remains instruction-level for proper debugger functionality
- Added PPU test `test_advance_dots_returns_frame_after_full_frame` to verify frame detection
- All 238 tests pass
- Files modified: `cpu_6502.rs`, `main.rs`, `nes_front_end.rs`, `tests/ppu_2c02.rs`

### Session: December 19, 2025 (session 9)
- Completed all 5 phases of cycle-accurate emulation refactoring
- **Activated cycle-accurate mode** in frontend (`step_frame_cycle_accurate()` now used instead of `step_frame()`)
- Phase 1: CPU cycle-stepping state machine
  - Added `CpuCycleState` enum (FetchOpcode, Executing, Halted)
  - Added `step_cycle()` to CPU trait and Cpu6502 implementation
  - Added `CpuCycleResult` struct for cycle activity reporting
  - Added `is_mid_instruction()`, `is_halted()`, `halt_cycles()`, `get_cycles()` methods
  - Backward compatible - existing `step_instruction()` still works
- Phase 2: DMA Controller
  - Created `dma_controller.rs` module (~270 lines)
  - `DmaController` manages OAM DMA and DMC DMA state
  - `step_cycle()` executes one DMA cycle (read/write alternating for OAM)
  - Proper 513-514 cycle timing for OAM DMA with odd/even alignment
  - DMC DMA support (1-4 cycles)
- Phase 3: Master Scheduler
  - Added `step_master_cycle()` and `step_frame_cycle_accurate()` to NesConsole
  - PpuDma signals DMA halt cycles via shared cells
  - OAM DMA now properly halts CPU for 513-514 cycles
  - Wired up shared cells through NesConsoleBuilder
- Phase 4: PPU Integration Refinements
  - Added `advance_dots()`, `get_dot()`, `get_scanline()` to PPU trait
  - Master scheduler uses `advance_dots(3)` for precise control
- Phase 5: APU Cycle-Stepping
  - Added `step_cycle()` to APU trait - advances by one CPU cycle
  - Added `needs_dmc_dma()` and `provide_dmc_sample()` for external DMA handling
  - APU clocks DMC/triangle at CPU rate, pulses/noise at half rate
- Added 11 new unit tests (5 CPU cycle-stepping + 6 DMA controller)
- All 237 tests pass
- Files created: `dma_controller.rs`
- Files modified: `cpu.rs`, `cpu_6502.rs`, `tests/cpu_6502.rs`, `lib.rs`, `ppu_dma.rs`, `nes_console.rs`, `ppu.rs`, `ppu_2c02.rs`, `apu.rs`, `apu_rp2a03.rs`

### Session: December 19, 2025 (session 8)
- Investigated "Arbitrary Sprite Zero" test - DEFERRED (see DEF-004)
- Attempted fixes:
  1. Mark first in-range sprite as sprite0 → caused FAIL 1 regression
  2. Mark sprite at OAMADDR/4 as sprite0 → passes FAIL 1, fails FAIL 2
- Reverted changes, added DEF-004 defect entry
- Root cause: requires cycle-accurate OAMADDR timing during sprite evaluation
- All 226 tests pass (no new tests added)

### Session: December 19, 2025 (session 7)
- Attempted fix for PPU Rendering Flag Behavior FAIL 2 (two changes made, retained):
  1. Background tile fetches now occur when only sprites are enabled (`render_background()` called when ShowSprites=true)
  2. Sprite 0 hit detection deferred: potential hits are stored during rendering, but the actual flag is only set at the hit dot if BOTH ShowBackground AND ShowSprites are enabled at that moment
- AccuracyCoin "RENDERING FLAG BEHAVIOR" test still fails FAIL 2 - deferred (see DEF-003)
- Added 3 unit tests for rendering flag behavior:
  - `test_background_fetches_occur_when_only_sprites_enabled`
  - `test_no_background_fetches_when_rendering_disabled`
  - `test_sprite_evaluation_when_only_background_enabled`
- All 226 tests pass (3 new tests added)

### Session: December 19, 2025 (session 6)
- Fixed PPU palette RAM 6-bit read behavior
- Palette RAM reads now return only lower 6 bits from palette, upper 2 bits from PPU open bus
- This matches NES hardware: palette RAM stores 6-bit color indices (0-63)
- Added unit test `test_palette_read_returns_6_bits_with_open_bus_upper_bits`
- Updated existing palette tests to account for open bus behavior
- AccuracyCoin "PALETTE RAM QUIRKS" FAIL 5 now passes
- All 223 tests pass

### Session: December 19, 2025 (session 5)
- Fixed PPU Read Buffer behavior for palette RAM reads
- When reading from palette RAM ($3F00-$3FFF), the read buffer now gets updated with the underlying nametable data ($2F00-$2FFF)
- Added unit test `test_palette_read_updates_buffer_with_nametable_data`
- AccuracyCoin "PPU READ BUFFER" test now passes
- All 222 tests pass

### Session: December 19, 2025 (session 4)
- Implemented PPU Open Bus Decay - PPU data bus now decays over time (~600ms)
- Each bit decays independently, tracked via per-bit refresh timestamps
- Added `total_dots` counter and `open_bus_refresh_dots[8]` to PPU struct
- AccuracyCoin "PPU Register Open Bus" test now passes (FAIL 4 fixed)
- Added 6 new unit tests for open bus decay behavior
- All 221 tests pass

### Session: December 19, 2025 (session 3)
- Fixed APU Open Bus - write-only APU registers ($4000-$4013, $4017) now return shared data bus value instead of 0
- AccuracyCoin "DMA + OPEN BUS" FAIL 1 passes
- Investigated DMC DMA timing for FAIL 2 - attempted pre-instruction DMA check, rolled back (cycle-accurate timing needed)
- Fixed CI pipeline Python virtual environment issue
- All 216 tests pass

### Session: December 19, 2025 (session 2)
- Fixed CPU/Bus Open Bus - data bus now tracks last read/written value
- Investigated DMC IRQ timing for "Interrupt Flag Latency" test - no fix committed (requires further research)
- All 215 tests pass

### Session: December 19, 2025
- Archived cycle-accurate timing refactoring task
- Implemented "ROM IS NOT WRITABLE" fix - AccuracyCoin test passes
- Implemented PPU Open Bus - AccuracyCoin DUMMY WRITE CYCLE tests pass
- All 215 tests pass

### Session: December 15, 2025
- Implemented cycle-accurate timing refactoring (Phases 1-6)
- AccuracyCoin improved from 74 to 82/131
- Fixed audio desync caused by APU odd/even cycle tracking bug
- Added 5 new PPU timing unit tests (213 total tests passing)

---

## Authorship Ratio Tracking

| Date | Human % | Claude % | Notes |
|------|---------|----------|-------|
| Dec 13, 2025 | 100% | 0% | Project start (human foundation) |
| Dec 15, 2025 | ~95% | ~5% | CPU tests, timing refactoring |
| Dec 19, 2025 | ~93% | ~7% | ROM read-only, PPU open bus |
| Dec 19, 2025 | ~92% | ~8% | CPU/Bus open bus |
| Dec 19, 2025 | ~91% | ~9% | APU open bus, CI fix |
| Dec 19, 2025 | ~90% | ~10% | PPU open bus decay |
| Dec 19, 2025 | ~90% | ~10% | PPU read buffer fix |
| Dec 19, 2025 | ~89% | ~11% | PPU palette 6-bit read |
| Dec 19, 2025 | ~88% | ~12% | PPU rendering flag behavior |
| Dec 19, 2025 | ~87% | ~13% | CPU cycle-stepping state machine (Phase 1) |
| Dec 19, 2025 | ~86% | ~14% | DMA controller (Phase 2) |
| Dec 19, 2025 | ~85% | ~15% | Master scheduler (Phase 3) |
| Dec 19, 2025 | ~85% | ~15% | PPU integration refinements (Phase 4) |
| Dec 19, 2025 | ~84% | ~16% | APU cycle-stepping (Phase 5) |
| Dec 20, 2025 | ~83% | ~17% | Phase 2 is_pc_dirty bug fix |
| Dec 20, 2025 | ~82% | ~18% | Phase 3 NMI/IRQ polling |

---

## Notes

- Authorship headers must be maintained in all source files
- Use absolute Windows paths for all file operations
- Build with `cargo build --verbose`
- Run tests with `cargo test --verbose`
- Run full CPU test suite with `cargo test test_all_opcodes -- --ignored --nocapture`
