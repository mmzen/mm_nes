# Fix Arbitrary Sprite Zero (Hardware-Accurate Alias)

## Goal
Pass the "Arbitrary Sprite Zero" ROM tests with the hardware-accurate model:
- Sprite 0 is OAM index 0 by default.
- **Alias exception**: if sprite evaluation begins at a non-zero OAMADDR while rendering is enabled, the sprite at OAMADDR is treated as sprite 0 for hit/priority during that evaluation window.

## Background
Recent changes marked the *first copied* sprite in secondary OAM as sprite 0. This passes some tests but breaks the ROM test:
"Sprite 0 should trigger a sprite zero hit. No other sprite should."
The correct model is not “first visible” or “first processed,” but “the entry at the start OAMADDR.”

## Required Behavior
1) **Default identity**
   - Sprite 0 is the sprite at OAM index 0.

2) **Alias exception**
   - If sprite evaluation begins with OAMADDR != 0 and rendering is enabled, the sprite *at that OAMADDR entry* is treated as sprite 0 for hit/priority during that evaluation.
   - This is based on the starting OAMADDR **byte pointer**, not on the first visible sprite.

3) **No “first visible” fallback**
   - If the aliased sprite is off‑scanline or transparent, it does **not** create a sprite‑0 hit.
   - Later visible sprites do **not** become sprite‑0.

4) **OAMADDR handling**
   - Use the byte-level OAMADDR to select the aliased sprite (do not discard low bits).
   - Keep evaluation order behavior unchanged otherwise.

## Implementation Notes (Non‑Prescriptive)
- Derive a byte-level start index: `start = oam_addr` (0..255).
- Alias sprite index = `start / 4` (the entry whose first byte is at OAMADDR), regardless of visibility.
- Mark `sprite0 = true` **only** on:
  - OAM index 0 when OAMADDR == 0, OR
  - The aliased sprite index when OAMADDR != 0 and rendering is enabled.
- Do **not** promote other sprites to sprite0 based on secondary OAM slot or visibility.

## Acceptance Criteria
- "Arbitrary Sprite Zero" passes both subtests.
- Sprite‑0 hit occurs only when the true/aliased sprite overlaps a non‑transparent background pixel with rendering rules satisfied.
- No regressions in sprite overflow or priority ordering.

## Suggested Tests
- ROM: Arbitrary Sprite Zero.
- Unit test (OAMADDR = 0): only OAM index 0 can ever have sprite0=true.
- Unit test (OAMADDR != 0, rendering enabled): only the aliased OAM index has sprite0=true, even if another sprite is first visible.
- Unit test (aliased sprite off‑scanline): no sprite0 hit should occur.
