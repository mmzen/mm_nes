# PLAN.md - Development Tracker

This file tracks the development progress of mmnes, documenting what has been done, what is planned, and serving as a continuity document across sessions.

---

## Current Status

**Last updated**: December 15, 2025

The emulator is functional with good CPU accuracy. The human built the foundation, and Claude is now the primary contributor.

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
- **SingleStepTests framework**: Implemented comprehensive test infrastructure in `mmnes_core/src/tests/singlestep/`
- **CPU compliance**: Fixed CPU to pass 100% of SingleStepTests (2,560,000/2,560,000) — was ~30% before
- **README.md**: Updated to reflect the human-to-AI experiment framing

---

## In Progress

*Nothing currently in progress*

---

## Planned Work

*Awaiting direction from supervisor*

Potential areas for improvement (to be prioritized):
- [ ] Improve AccuracyCoin score (currently 74/131)
- [ ] MMC3 mapper support
- [ ] Cycle-accurate PPU (mid-scanline updates)
- [ ] Save states
- [ ] Controller 2 support
- [ ] Breakpoints & watchpoints in debugger
- [ ] Memory inspector
- [ ] PPU/APU visualizers
- [ ] APU filters

---

## Session Log

### Session: December 15, 2025
- Read and understood CLAUDE.md guidelines
- Updated README.md to reflect the human-to-AI software engineering experiment
- Added contributions timeline section crediting human and Claude work
- Created PLAN.md for development tracking

---

## Notes

- Authorship headers must be maintained in all source files
- Use absolute Windows paths for all file operations
- Build with `cargo build --verbose`
- Run tests with `cargo test --verbose`
- Run full CPU test suite with `cargo test test_all_opcodes -- --ignored --nocapture`
