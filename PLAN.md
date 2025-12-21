# PLAN.md - Development Tracker

This file tracks the development progress of mmnes, documenting what has been done, what is planned, and serving as a continuity document across sessions.

---

## Current Status

**Last updated**: December 21, 2025

The emulator is functional with **true cycle-accurate emulation**. All 8 phases of cycle accuracy roadmap complete.

**Honest characterization**:
- CPU: **Cycle-accurate bus operations** with **interrupt polling on each cycle**
- PPU: **Dot-level rendering** - background shift registers, per-dot pixel output, mid-scanline register effects
- DMA: **Cycle-by-cycle OAM DMA** + **DMC DMA mid-instruction stealing** (1-4 cycles based on CPU state)
- Scheduler: Cycle-synchronized with DMC DMA checked every cycle
- Interrupts: **Latched polling** - NMI/IRQ sampled at start of each cycle, used at instruction completion

**Current focus**: None (Convergence Phase on hold)

**AccuracyCoin Score**: 90/131 (target: 120+/131)

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
| Phase 4: DMC DMA Mid-Instruction | **DMC DMA steals 1-4 cycles based on CPU state** | Integrated with scheduler, 4 new tests |
| Phase 5: PPU Dot-Level Rendering | **Per-dot pixel output, shift registers, mid-scanline effects** | Background alignment fix, 7 new tests |
| Phase 6: Odd Frame Skip | **89341 vs 89342 dots per frame (NTSC)** | Frame parity tracking, 3 new tests |
| Phase 7: CPU/PPU Alignment | **Verified 0 drift over 1000 frames** | Perfect 3:1 PPU:CPU ratio |
| Phase 8: APU Frame Counter | **Exact hardware cycle values for NTSC** | 4-step/5-step timing, 3 new tests |
| Singlestep Tests Cycle-Accurate | **Tests now use step_cycle(), 100% pass rate** | SHA/SHX/SHY, JAM, TAS fixes |
| AccuracyCoin OPEN BUS Fixes | **$4015 open bus behavior** | FAIL 7 & FAIL 9 fixed |

---

## In Progress

*No active tasks*

---

## On Hold

### Convergence Phase: Test ROM Fixes

**Status**: On hold (December 21, 2025)
**Goal**: Fix remaining AccuracyCoin failures using cycle-accurate infrastructure
**Current Score**: 90/131 | **Target**: 120+/131
**Details**: [claude_directory/convergence_phase_on_hold.md](claude_directory/convergence_phase_on_hold.md)

**Last work**: Investigated DMC DMA + OAM DMA test - DMASync occasionally succeeds but test still fails overall. Debug statements removed, code in clean state.

## Cycle Accuracy Roadmap - COMPLETED

**Status**: ✓ All 8 phases complete (December 20, 2025)

**Goal achieved**: True cycle-accurate emulation with per-cycle bus modeling, dot-level PPU, sub-instruction interrupt polling, and cycle-by-cycle DMA.

| Phase | Description | Key Outcome |
|-------|-------------|-------------|
| 1 | DmaController Wiring | OAM DMA cycle-by-cycle (513/514 cycles) |
| 2 | CPU Bus-Cycle Modeling | Each `step_cycle()` = one bus operation |
| 3 | NMI/IRQ Polling | Latched polling at cycle start |
| 4 | DMC DMA Mid-Instruction | Steals 1-4 cycles based on CPU state |
| 5 | PPU Dot-Level Rendering | Per-dot pixels, shift registers, mid-scanline effects |
| 6 | Odd Frame Skip (NTSC) | 89341 vs 89342 dots per frame |
| 7 | CPU/PPU Alignment | Verified 0 drift over 1000 frames |
| 8 | APU Frame Counter | Exact hardware cycle values |

**Full implementation details**: [claude_directory/cycle_accuracy_roadmap_phases_1_8.md](claude_directory/cycle_accuracy_roadmap_phases_1_8.md)

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

### DEF-005: CPU Bus Phase Timing (phi1/phi2)
**Description**: Several AccuracyCoin tests require sub-cycle bus phase timing that's not currently implemented.
**Affected tests**:
- FRAME COUNTER IRQ FAIL 7: "IRQ flag should not be cleared when APU transitions from 'get' to 'put' cycle"
- CONTROLLER STROBING FAIL 4: "Controllers should not be strobed when CPU transitions from 'put' to 'get' cycle"
- CONTROLLER CLOCKING FAIL 3: "Double-reading $4016 should only clock controller once"
**Root cause**: The NES CPU has two phases per cycle: phi1 (address phase / "get") and phi2 (data phase / "put"). Side effects like clearing IRQ flags or clocking shift registers should only occur during phi2.
**Required changes**:
- Add bus phase tracking (phi1/phi2) to CPU cycle state
- Modify Memory trait to distinguish between "address phase read" and "data phase read"
- Update APU and controller implementations to only perform side effects during phi2
**Notes**: This is a significant architectural change. Deferred for future work.

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

### Session: December 21, 2025 (session 22)
- Archived Convergence Phase task (put on hold)
- Removed all debug statements from DMC DMA investigation
- Code restored to clean state
- Files cleaned: `nes_bus.rs`, `dma_controller.rs`, `nes_console.rs`, `apu_rp2a03.rs`
- All 279 tests pass

### Session: December 20, 2025 (sessions 10-21)
- **Completed all 8 phases of cycle accuracy roadmap**
- Key milestones:
  - Phase 1-2: DMA controller wiring, CPU bus-cycle modeling
  - Phase 3-4: NMI/IRQ polling, DMC DMA mid-instruction
  - Phase 5: PPU dot-level rendering with shift registers
  - Phase 6-8: Odd frame skip, CPU/PPU alignment, APU frame counter
- SingleStep tests: 100% pass rate (2,560,000/2,560,000)
- Fixed DMC DMA data bus issue (external DMA handling)
- Fixed Punch-Out sprite regression
- Full details in [claude_directory/](claude_directory/) archive files

### Sessions: December 15-19, 2025 (sessions 1-9)
- Initial cycle-accurate timing refactoring
- PPU/APU/CPU open bus fixes
- AccuracyCoin score: 74 → 90/131
- Full details in [claude_directory/](claude_directory/) archive files

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
| Dec 20, 2025 | ~81% | ~19% | Phase 4 DMC DMA mid-instruction |
| Dec 20, 2025 | ~79% | ~21% | Phase 5 COMPLETE (shift registers, per-dot rendering, mid-scanline effects) |
| Dec 20, 2025 | ~78% | ~22% | Singlestep tests cycle-accurate, SHA/SHX/SHY fixes |
| Dec 20, 2025 | ~77% | ~23% | Phase 6 (odd frame skip), AccuracyCoin OPEN BUS fixes |
| Dec 20, 2025 | ~77% | ~23% | Phase 7 (CPU/PPU alignment verification) - no code changes needed |
| Dec 20, 2025 | ~76% | ~24% | Phase 8 (APU frame counter) - ALL 8 PHASES COMPLETE! |
| Dec 20, 2025 | ~76% | ~24% | APU timing fixes (cpu_cycle_odd, DMC restart) |
| Dec 20, 2025 | ~76% | ~24% | DMC DMA data bus fix (INC $4014 + IMPLIED DUMMY READ hangs) |
| Dec 21, 2025 | ~76% | ~24% | Debug cleanup, Convergence Phase archived |

---

## Notes

- Authorship headers must be maintained in all source files
- Use absolute Windows paths for all file operations
- Build with `cargo build --verbose`
- Run tests with `cargo test --verbose`
- Run full CPU test suite with `cargo test test_all_opcodes -- --ignored --nocapture`
