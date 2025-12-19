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
| CPU/Bus Open Bus | AccuracyCoin OPEN BUS FAIL 1-2 pass | Data bus tracking in NESBus |
| DMC IRQ Timing | AccuracyCoin Interrupt Flag Latency | IRQ now fires when bytes_remaining becomes 0 |

---

## In Progress

*No active tasks*

---

## Planned Work

*No planned tasks*

---

## Known Defects

| ID | Description | Severity | Status |
|----|-------------|----------|--------|
| DEF-001 | Audio crackles/pops during playback | Low | Open |

### DEF-001: Audio Crackles
**Description**: Occasional audio crackles/pops during emulation playback.
**Possible causes**:
- Sample buffer underruns in the audio pipeline
- Timing jitter from fine-grained synchronization
- SDL2 audio callback timing issues
**Notes**: Audio is synchronized with video; only quality is affected.

---

## Session Log

### Session: December 19, 2025 (continued)
- Fixed CPU/Bus Open Bus - data bus now tracks last read/written value
- Fixed DMC IRQ timing - IRQ now triggers when bytes_remaining becomes 0 (when last byte is read), not when all bits are output
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
| Dec 19, 2025 | ~92% | ~8% | CPU/Bus open bus, DMC IRQ timing |

---

## Notes

- Authorship headers must be maintained in all source files
- Use absolute Windows paths for all file operations
- Build with `cargo build --verbose`
- Run tests with `cargo test --verbose`
- Run full CPU test suite with `cargo test test_all_opcodes -- --ignored --nocapture`
