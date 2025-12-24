# FOLLOW-UP: VBlank Beginning boundary read suppression

## Context
The earlier test only checked that $2002 shows VBlank at scanline 241, dot 1.  
The ROM failure shows a **boundary case**: a $2002 read on the same master cycle that would set VBlank must **suppress** the VBlank set (and NMI) for that frame.

This follow-up test must reproduce that boundary behavior.

## Source behavior (from ROM)
- When the first $2002 read is **too early**: X=$00, and the next read happens after VBlank begins so Y=$80.
- When the first $2002 read is **on the same cycle** that would set VBlank: X=$00 and Y=$00, and **VBlank is not set afterwards**.
- When the first $2002 read is **after** VBlank begins: X=$80 and Y=$00 (since the read clears it).

## Test goal
Prove the CPU-visible behavior at the exact boundary:
- Reading $2002 on the cycle that would set VBlank must return bit 7 = 0 **and** prevent VBlank from being set for that frame.

## Test requirements
### 1) Use real scheduler ordering
- The test must use `NesConsole::step_master_cycle()` so CPU/PPU ordering is identical to production.

### 2) Target the boundary cycle
Implement a deterministic harness that:
- Advances the console to the frame where the PPU will reach **scanline 241, dot 1**.
- Schedules **two consecutive CPU reads** of $2002 on adjacent master cycles.
- Forces the first read to occur on the **same master cycle** that would otherwise set VBlank.

### 3) Assertions
On the boundary case (first read on dot 1):
- Read #1: bit 7 = 0.
- Read #2 (next CPU cycle): bit 7 = 0.
- After both reads, PPU VBlank flag remains **clear** for that frame (do not set VBlank later in the same scanline).

On the immediate-after case (first read one cycle later):
- Read #1: bit 7 = 1.
- Read #2: bit 7 = 0 (cleared by the read).

### 4) Output
Add the test under `mmnes_core/src/tests/` and keep it self-contained.  
Use a minimal iNES ROM (generated in the test) and a test CPU that only reads $2002.

## Expected status
- This test **must fail** on current code (VBlank is always set at dot 1).
- It should pass once boundary read suppression is implemented.
