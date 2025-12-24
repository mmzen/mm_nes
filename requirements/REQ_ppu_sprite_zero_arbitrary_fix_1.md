# Fix Sprite 0 Attribution Regression

## Goal
Restore correct NES PPU sprite-0 hit semantics so only the sprite originating from OAM index 0 can trigger a sprite-0 hit, even when sprite evaluation starts from a non-zero OAMADDR.

## Background
Current failure:
- "Arbitrary Sprite Zero" test fails with: "1: Sprite 0 should trigger a sprite zero hit. No other sprite should."
- Regression appears in `mmnes_core/src/ppu_2c02.rs`, where the first processed sprite (secondary OAM slot 0) is flagged as sprite 0.

## Scope
- PPU sprite evaluation and sprite-0 hit logic.

## Requirements
1) Sprite-0 attribution must be based on original OAM index 0, not "first processed sprite."
2) If evaluation starts at OAMADDR != 0, sprite-0 hit should occur only if the sprite from OAM index 0 is copied into secondary OAM.
3) Maintain current evaluation order behavior if needed for other timing tests, but decouple it from sprite-0 attribution.
4) Update or replace any unit tests that assert "first processed sprite becomes sprite 0" so they match correct behavior.

## Implementation Notes
- In `do_sprite_evaluation`, mark `sprite0 = true` only when the sprite's original OAM index is 0 (e.g., `i == 0`), regardless of `count`.
- If needed, store original OAM index alongside copied sprites or set a separate flag when the OAM index 0 sprite is selected.

## Acceptance Criteria
- The ROM test "Arbitrary Sprite Zero" passes, specifically step 1: "Sprite 0 should trigger a sprite zero hit. No other sprite should."
- No other PPU tests regress.
- New/modified unit tests reflect correct sprite-0 attribution and pass.

## Suggested Tests
- Run the existing Arbitrary Sprite Zero test ROM.
- Add/update unit tests to assert:
  - OAM index 0 triggers sprite-0 hit.
  - A non-zero OAM index does not trigger sprite-0 hit even if it is the first processed sprite.
