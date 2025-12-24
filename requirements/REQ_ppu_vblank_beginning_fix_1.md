# REQUIREMENT (FOLLOW-UP): Fix VBlank boundary suppression + test reliability

## Context
The current boundary suppression implementation and test still fail.  
The ROM expects **sub-dot ordering**: a $2002 read that occurs *after* dot 1 must see VBlank set, while a read *on* the dot-1 boundary must suppress VBlank for that frame.

## Required changes

### 1) Scheduler sub-dot ordering (must)
Update `NesConsole::step_master_cycle()` so the CPU bus op is ordered between PPU dots:
- Advance **1 PPU dot**,
- Execute CPU/DMA bus op,
- Advance the remaining **2 PPU dots** (NTSC).

This gives a concrete dot phase for the CPU read and allows the ROM’s A=4/A=5 cases to diverge.
Do **not** remove DMA ordering or violate the one-bus-op-per-cycle invariant.

### 2) Boundary suppression should be dot-accurate (must)
`$2002` boundary suppression must only trigger when the read happens on the **same dot** that would set VBlank (scanline 241, dot 1).
If the read happens after dot 1 in that master cycle, VBlank must already be set.

### 3) Reset of boundary-read latch (must)
If a `status_read_this_cycle`-style latch is used:
- It must be cleared regardless of whether the PPU is advanced via `run()` or `advance_dots()` (avoid state leaks).

### 4) NMI suppression duration (should)
If boundary suppression occurs, NMI must be suppressed for that VBlank event (frame).
Do not immediately clear the suppression in the same dot-1 block.

## Test reliability requirements
The boundary test must be deterministic and guaranteed to hit the boundary read:
- Use `power_on_deterministic()` instead of `power_on()` to eliminate random phase.
- Search enough frames or compute alignment so a `$2002` read **must** land on the boundary.
- Do not allow “failed to find boundary read” as a possible outcome.

## Acceptance criteria
- Boundary suppression test passes deterministically.
- ROM no longer reports “VBlank Beginning” failure.
- Existing PPU timing tests still pass (scanline 241 dot 1 and pre-render clearing).
