# Fix MMC3 A12 Notification Source

## Goal
Eliminate double-counted A12 edges and align MMC3 IRQ timing with hardware by making **CHR memory** the sole source of `notify_ppu_address` calls. The PPU must not call `cartridge.notify_ppu_address()` directly.

## Required Behavior
1) **Single notification source**
   - Remove or disable the PPU’s direct `notify_ppu_address` calls.
   - Keep A12 notifications inside the MMC3 CHR memory implementation (`Mmc3ChrMemory::read_byte/write_byte`).

2) **No double-clocked IRQs**
   - Ensure each PPU CHR fetch results in **one** A12 observation for the MMC3.
   - The A12 low-time filter must not be accidentally bypassed by duplicate notifications.

3) **Tests updated to validate correctness**
   - Add at least one IRQ timing test that exercises actual PPU CHR fetches (or a proxy path that triggers CHR memory reads) to ensure A12 edges are counted once.
   - Add a PRG/CHR bank mapping test that reads via CPU/PPU address paths, not only register state.

## Acceptance Criteria
- MMC3 IRQ tests still pass, and no double-trigger behavior is observed.
- Bank mapping is verified via real address reads (CPU for PRG, PPU/CHR memory for CHR).
- All existing MMC3 tests remain green.

## Notes
- If the emulator uses PPU bus reads to access CHR memory, keeping notifications in `Mmc3ChrMemory` is sufficient and avoids double-counting.
- If other mappers also rely on `notify_ppu_address`, confirm they still receive notifications through CHR memory access paths.
