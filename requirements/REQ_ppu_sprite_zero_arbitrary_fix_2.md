# Fix Arbitrary Sprite Zero - Test 2

## Goal
Pass "Arbitrary Sprite Zero, 2: The first processed sprite of a scanline should be treated as 'sprite zero'" without regressing test 1.

## Background
- Current behavior ties sprite-0 hit strictly to OAM index 0 in `do_sprite_evaluation`.
- The test expects the first sprite evaluated (based on current OAMADDR and evaluation order) to be treated as sprite zero for that scanline.
- There are likely OAMADDR timing inaccuracies that also affect which sprite is processed first:
  - Writes to $2004 during rendering currently advance OAMADDR by 4, not 1.
  - OAMADDR is reset at dot 257 unconditionally, even if rendering is disabled.
  - Low bits of OAMADDR are discarded when selecting the start sprite.

## Required Behavior
1) **Sprite-0 attribution follows evaluation order.**
   - The first sprite copied into secondary OAM for a scanline must be flagged as `sprite0 = true`.
   - All other copied sprites must have `sprite0 = false`.
2) **Evaluation order respects OAMADDR as a byte pointer.**
   - Start sprite evaluation from OAMADDR (byte address) and wrap across 256 bytes.
   - If OAMADDR is unaligned (low 2 bits set), the byte-level offset must influence which sprite becomes "first processed."
3) **OAMADDR behavior matches hardware during rendering.**
   - Writes to $2004 during rendering do not modify OAM contents but increment OAMADDR by 1, not 4.
   - OAMADDR reset at dot 257 should occur only when rendering is enabled.

## Implementation Notes (Non-Prescriptive)
- Track evaluation start at the byte level (0..255) and derive sprite index and byte offset from it.
- When the first in-range sprite is copied to secondary OAM (slot 0), set `sprite0 = true`.
- Ensure existing sprite overflow behavior remains unchanged.

## Acceptance Criteria
- "Arbitrary Sprite Zero, 2" passes.
- "Arbitrary Sprite Zero, 1" still passes.
- No regressions in sprite overflow or general sprite rendering tests.

## Suggested Tests
- Run the Arbitrary Sprite Zero test ROM (both sub-tests).
- Add a unit test that sets OAMADDR to a non-zero, non-aligned value before evaluation and asserts that:
  - Secondary OAM slot 0 is sprite0.
  - Its source corresponds to the first processed sprite given the byte-level OAMADDR.
- Add/keep a unit test where OAMADDR=0 and confirm sprite 0 is still the first processed sprite.
