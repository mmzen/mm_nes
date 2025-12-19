# PLAN.md - Development Tracker

This file tracks the development progress of mmnes, documenting what has been done, what is planned, and serving as a continuity document across sessions.

---

## Current Status

**Last updated**: December 19, 2025

The emulator is functional with good CPU accuracy and improved PPU timing. The human built the foundation, and Claude is now the primary contributor.

**Current focus**: AccuracyCoin test compliance - working through failing tests.

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
- AccuracyCoin score: 74/131

### Claude Contributions (December 2025 → Present)

| Task | Outcome | Details |
|------|---------|---------|
| SingleStepTests framework | CPU passes 100% (2,560,000/2,560,000) | `mmnes_core/src/tests/singlestep/` |
| README.md update | Reflects human-to-AI experiment framing | - |
| Cycle-accurate timing refactoring | AccuracyCoin 74→82/131, audio synced | [Full details](claude_directory/cycle_accurate_timing_refactoring.md) |
| ROM IS NOT WRITABLE fix | AccuracyCoin test passes | [Full details](claude_directory/rom_is_not_writable.md) |
| PPU Open Bus | AccuracyCoin DUMMY WRITE CYCLE passes | [Full details](claude_directory/ppu_open_bus.md) |
| CPU/Bus Open Bus | AccuracyCoin OPEN BUS FAIL 1 passes | Data bus tracking in NESBus |
| APU Open Bus | AccuracyCoin DMA + OPEN BUS FAIL 1 passes | [Full details](claude_directory/apu_open_bus.md) |
| PPU Open Bus Decay | AccuracyCoin "PPU Register Open Bus" passes | [Full details](claude_directory/ppu_open_bus_decay.md) |
| PPU Read Buffer | AccuracyCoin "PPU READ BUFFER" passes | Palette read now updates buffer with nametable data |
| PPU Palette 6-bit | AccuracyCoin "PALETTE RAM QUIRKS" FAIL 5 passes | Palette reads return 6-bit value + open bus upper 2 bits |

---

## In Progress

*No active tasks*

## Planned Work

*No planned tasks*

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

---

## Notes

- Authorship headers must be maintained in all source files
- Use absolute Windows paths for all file operations
- Build with `cargo build --verbose`
- Run tests with `cargo test --verbose`
- Run full CPU test suite with `cargo test test_all_opcodes -- --ignored --nocapture`
