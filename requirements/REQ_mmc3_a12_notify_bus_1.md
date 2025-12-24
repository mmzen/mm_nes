# MMC3 A12 Low-Time Tracking Without PPU-Driven Notify

## Goal
Keep PPU from directly calling `notify_ppu_address` while ensuring MMC3 A12 low-time filtering still reflects **all** PPU bus activity (including nametable/attribute fetches), so IRQs fire correctly in real games.

## Problem
With PPU notifications removed, A12 low-time is now only tracked on CHR memory reads/writes. This can miss the low period provided by nametable/attribute fetches (0x2000+), especially when both BG and sprites use the $1000 pattern table, leading to missed IRQ clocks.

## Required Behavior
1) **PPU must not call `notify_ppu_address` directly**
   - No direct calls from PPU code paths.

2) **A12 tracking must observe all PPU bus accesses**
   - A12 low/high state must be updated for every PPU bus read/write (pattern tables, nametables, palette, etc.).
   - The low-time filter must behave as if real PPU cycles are being observed (no double-counting).

3) **Single source of notification**
   - Ensure each PPU bus access results in exactly one A12 observation.
   - Avoid duplicated notifications (PPU + CHR memory) which halve the low-time filter.

4) **Documentation consistency**
   - Update comments/docs that still claim PPU directly calls `notify_ppu_address`.

## Implementation Notes (Non‑Prescriptive)
- Move A12 notification to a **PPU bus-level hook** (e.g., NESBus wrapper or dedicated observer device) so *all* PPU bus reads/writes trigger exactly one `notify_ppu_address` call.
- Keep `Mmc3ChrMemory` free of direct notifications if the bus is the single observer.
- If keeping notifications in `Mmc3ChrMemory`, add equivalent notifications for non‑CHR PPU accesses via a dedicated observer device to cover A12 low time from nametable/attribute fetches.

## Acceptance Criteria
- MMC3 IRQ timing matches expected behavior in scanline IRQ test ROMs.
- No double‑clocked IRQs.
- IRQs still fire when both BG and sprites use the $1000 pattern table.

## Suggested Tests
- Unit test: simulate PPU bus reads to nametable addresses (0x2000+) followed by a CHR read at 0x1000 and assert the A12 low filter qualifies the edge.
- Unit test: ensure only one A12 observation per PPU bus access (no double notification).
- Run MMC3 IRQ test ROMs if available.
