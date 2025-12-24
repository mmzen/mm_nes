# REQUIREMENT: Arbitrary Sprite Zero (first processed sprite)

## Context
Failing test: "Arbitrary Sprite Zero, 2: The first processed sprite of a scanline should be treated as 'sprite zero'."

Current implementation in `mmnes_core/src/ppu_2c02.rs` sets the `sprite0` flag only when the primary OAM index is 0:
- `Ppu2c02::do_sprite_evaluation()` uses `if i == 0 { sprite0 = true; }`

This ignores the **first processed sprite** for the scanline and does not allow tests that shift the sprite evaluation order (e.g., via OAMADDR) to pass.

## Required behavior
1. **First processed sprite is sprite zero for that scanline.**
   - The first sprite copied into secondary OAM (slot 0) must be marked as `sprite0 = true`.
   - All other sprites in secondary OAM must have `sprite0 = false`.

2. **Sprite evaluation order must respect the current OAMADDR.**
   - If OAMADDR is not zero at the start of sprite evaluation, the evaluation must begin at that sprite index and wrap around.
   - This determines which sprite is "first processed" for the scanline.

3. **Per-scanline reset.**
   - The `sprite0` designation must be recomputed each scanline (do not carry it between scanlines).

## Implementation guidance (non-prescriptive)
- In `Ppu2c02::do_sprite_evaluation()`:
  - Compute a starting sprite index from `oam_addr` (byte address), e.g. `start = oam_addr / 4`.
  - Iterate 64 sprites with wrapping (start..start+63), copying sprites in that order.
  - When `count == 0` (first sprite copied into secondary OAM), set `sprite0 = true`.

## Acceptance criteria
- The "Arbitrary Sprite Zero, 2" test passes.
- Existing sprite 0 hit behavior is preserved for normal OAMADDR=0 cases.
- No regressions in sprite overflow or sprite 0 hit timing tests.

## Tests (must)
Add 1+ unit tests under `mmnes_core/src/tests/` that:
1. **Reveal the current bug**: with a non-zero OAMADDR at the start of sprite evaluation, the current code still marks primary sprite index 0 as sprite zero, instead of the first processed sprite.
2. **Validate the fix**: after the change, the first sprite copied into secondary OAM is marked `sprite0 = true` and no others are.

Suggested test structure:
- Build a PPU instance with a known primary OAM layout (e.g., sprite 0 and sprite N overlap the scanline).
- Set `oam_addr` to point at sprite N before calling `do_sprite_evaluation(scanline)`.
- Assert that secondary OAM slot 0 has `sprite0 = true` and that the sprite corresponds to the first processed entry (sprite N).
- Also assert that when `oam_addr = 0`, sprite 0 remains the designated sprite zero.
