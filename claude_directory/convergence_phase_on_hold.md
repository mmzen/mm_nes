# Convergence Phase - ON HOLD

**Status**: On Hold (December 21, 2025)
**Goal**: Systematically fix remaining failures in test ROMs (AccuracyCoin, blargg, etc.) using the cycle-accurate infrastructure built in Phases 1-8.
**Target**: 120+/131 AccuracyCoin score (currently 90/131)

---

## Approach

1. Run test ROM, identify failure code
2. Research expected hardware behavior (NESDev wiki, docs)
3. Implement fix
4. Verify with unit tests
5. Repeat until test passes

---

## Known Remaining AccuracyCoin Failures

From 90/131 score:

- **CPU Behavior**: FAIL 2 (DUMMY WRITE CYCLES), FAIL 6 (OPEN BUS), FAIL A (UNOFFICIAL INSTRUCTIONS)
- **DMA + OPEN BUS**: FAIL 2
- **RENDERING FLAG BEHAVIOR**: FAIL 2
- **ARBITRARY SPRITE ZERO**
- Others TBD

---

## Session 21 Investigation: DMC DMA + OAM DMA Test

### Problem
AccuracyCoin "DMC DMA + OAM DMA" test fails despite the DMASync loop occasionally succeeding.

### Verified Behavior
- DMA reads DO go through NESBus::read_byte (same bus pointer confirmed via debug output)
- data_bus IS updated when DMA reads PRG ROM (confirmed via debug statements)
- SUCCESS messages appear when timing aligns (CPU at cycle 3 of LDA $4000)
- Both NESBus and APU share the same data_bus pointer (e.g., `0x1c782dbc8a8`)

### Debug Added (Now Removed)
- `nes_bus.rs`: Specific tracking for DMC sample address (0xFFC0) reads
- `dma_controller.rs`: Bus pointer tracking and DMA read logging
- `nes_console.rs`: Cycle-after-DMA tracking, OAM DMA start logging
- `apu_rp2a03.rs`: $4000 read tracking with pointer info, SUCCESS messages when data_bus=0x00

### Observations
- DMASync loop sometimes succeeds (SUCCESS messages appear)
- But test still hangs/fails overall
- Need to investigate: Why does test fail despite individual DMASync successes?

### Open Questions
1. Why does the test fail overall despite individual DMASync successes?
2. Does the test have multiple phases requiring consistent timing?
3. Is it failing in a later section after initial DMASync passes?

---

## Session 20: DMC DMA Data Bus Fix

### Root Cause Identified
AccuracyCoin "INC $4014" and "IMPLIED DUMMY READ" tests hang in DMA sync loop:
- Tests use `DMASyncWith*` functions that loop waiting for DMC DMA to update the data bus
- Loop: `LDA $4000` returns open bus ($40 = high byte) until DMC DMA puts sample on bus
- If DMC DMA doesn't update data bus, loop is infinite

### The Fix
Enable external DMC DMA handling in `step_master_cycle()`:
- Set `apu.set_external_dmc_dma(true)` to disable internal APU DMA
- Check `apu.needs_dmc_dma()` each cycle to detect when sample buffer needs refill
- Start DMC DMA via `dma_controller.start_dmc_dma()` which reads through the bus
- Bus reads update the shared `data_bus` value
- When DMC DMA completes, call `apu.provide_dmc_sample()` with fetched byte

### Additional Fix: DMC+OAM DMA Interleaving
- Initial fix caused new hangs in "DMC DMA + OAM DMA" and "INSTRUCTION TIMING" tests
- Root cause: DMC DMA was given full priority, pausing OAM DMA completely
- Per AccuracyCoin: during DMC halt cycles, OAM DMA should continue
- Only DMC "get" cycle (final read) needs exclusive bus access
- Fixed `step_cycle()` to run both DMAs during halt cycles, pause OAM only for get cycle

---

## Known Defects Related to This Phase

### DEF-002: DMC DMA Timing
AccuracyCoin "DMA + OPEN BUS" FAIL 2 - DMC DMA occurs on wrong cycle or doesn't update data bus correctly.

### DEF-003: Rendering Flag Behavior FAIL 2
Background shift registers should be initialized and clocked when only rendering sprites.

### DEF-004: Arbitrary Sprite Zero
Test requires precise timing coordination between OAMADDR writes and sprite evaluation.

### DEF-005: CPU Bus Phase Timing (phi1/phi2)
Several tests require sub-cycle bus phase timing:
- FRAME COUNTER IRQ FAIL 7
- CONTROLLER STROBING FAIL 4
- CONTROLLER CLOCKING FAIL 3

---

## Files Modified During This Phase

- `nes_bus.rs` (Human 90% | Claude 10%)
- `dma_controller.rs` (Human 0% | Claude 100%)
- `nes_console.rs` (Human 40% | Claude 60%)
- `apu_rp2a03.rs` (Human 65% | Claude 35%)

---

## Resume Notes

When resuming this task:
1. All debug statements have been removed - code is in clean state
2. Next step: Investigate why DMC DMA + OAM DMA test fails despite individual DMASync successes
3. Consider adding logging selectively to understand multi-phase test behavior
4. May need to analyze AccuracyCoin source code more deeply for test expectations
