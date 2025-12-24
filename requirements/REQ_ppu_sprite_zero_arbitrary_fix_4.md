# Fix Arbitrary Sprite Zero (Rendering-Gated OAMADDR + Byte-Level Evaluation)

## Goal
Align PPU sprite evaluation and sprite‑0 aliasing with documented hardware behavior so the Arbitrary Sprite Zero ROM passes reliably, including unaligned OAMADDR cases.

## Required Behavior
1) **Gate OAMADDR reset on rendering**
   - OAMADDR is reset to 0 during ticks 257–320 **only when rendering is enabled** (BG or sprites on).
   - If rendering is disabled, OAMADDR must be preserved (critical for mid‑frame enable aliasing).

2) **Byte‑level sprite evaluation**
   - Sprite evaluation starts at the **byte** pointed to by OAMADDR (0..255), not at `oam_addr / 4`.
   - If OAMADDR is unaligned (not a Y byte), evaluation must reinterpret bytes accordingly (Y/TILE/ATTR/X shift), as documented on nesdev.
   - The evaluation order must wrap across all 256 bytes of OAM.

3) **Sprite‑0 identity + alias exception**
   - Default: sprite‑0 is OAM index 0.
   - Alias exception: when rendering is enabled and evaluation starts at non‑zero OAMADDR, the sprite entry that begins at that OAMADDR byte is treated as sprite‑0 for hit/priority.
   - There is **no** “first visible sprite” fallback. If the aliased sprite is off‑scanline/transparent, no sprite‑0 hit occurs.

4) **Evaluation only when rendering is enabled**
   - Sprite evaluation occurs if **either** background or sprites are enabled, and stops only if **both** are disabled.

## Implementation Notes (Non‑Prescriptive)
- Track a byte‑level evaluation cursor (0..255). Derive a logical sprite index and byte offset from it for each candidate.
- Apply aliasing based on the starting OAMADDR byte, not on the first visible or first copied sprite.
- Gate OAMADDR reset points (pre‑render and visible scanlines) on `ShowBackground || ShowSprites`.
- Do **not** implement revision‑specific OAM corruption yet unless a PPU revision toggle is introduced.

## Acceptance Criteria
- Arbitrary Sprite Zero ROM passes (both subtests).
- Aliased sprite‑0 behavior works with unaligned OAMADDR.
- No regressions in sprite overflow or general rendering behavior.

## Suggested Tests
- Unit test: OAMADDR unaligned (e.g., 0x01/0x02/0x03), verify byte‑level reinterpretation and correct aliasing.
- Unit test: OAMADDR preserved while rendering off, then aliasing triggers when rendering enabled mid‑frame.
- ROM: Arbitrary Sprite Zero.
