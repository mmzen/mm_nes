# Fix Arbitrary Sprite Zero (Pre-render OAMADDR + Unaligned Tests)

## Goal
Close remaining gaps in sprite‑0 aliasing by aligning pre‑render behavior with the documented OAMADDR reset window and by tightening tests around the unaligned OAMADDR case.

## Required Behavior
1) **Pre‑render OAMADDR reset timing**
   - Do **not** reset OAMADDR at pre‑render dot 1 unless it corresponds to the documented rendering window behavior (ticks 257–320 while rendering is enabled).
   - If rendering is disabled, OAMADDR must be preserved through pre‑render and into visible scanlines.

2) **No early sprite evaluation side‑effects**
   - If sprite evaluation is triggered at pre‑render dot 1 for scanline 0, it must not force‑reset OAMADDR in a way that breaks aliasing when rendering begins.

3) **Unaligned OAMADDR test must assert no fallback**
   - Extend the unaligned OAMADDR unit test to assert that when the aliased virtual sprite is off‑scanline, **no** sprite gets `sprite0=true`.

4) **Documentation cleanup**
   - Update sprite‑zero test header comments to reflect the alias model (OAM index 0 by default, OAMADDR alias exception when rendering begins at non‑zero).

## Acceptance Criteria
- Arbitrary Sprite Zero ROM passes.
- Pre‑render OAMADDR handling no longer wipes a non‑zero OAMADDR while rendering is disabled.
- Unaligned OAMADDR test explicitly guards against sprite‑0 fallback.

## Suggested Tests
- Run Arbitrary Sprite Zero ROM.
- Update the unaligned OAMADDR unit test to assert `sprite0=false` when aliased virtual sprite is off‑scanline.
- Add a unit test that sets OAMADDR in VBlank, enables rendering mid‑frame, and verifies aliasing survives pre‑render.
