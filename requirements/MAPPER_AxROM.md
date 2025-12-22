# AxROM (Mapper 7) Requirements

Goal: add Mapper 7 (AxROM: AOROM/AMROM/ANROM/ASROM) support to `mmnes_core` in a way that matches the existing cartridge/memory/bus architecture.

## Scope

- Implement a new AxROM cartridge module.
- Wire it into mapper selection and cartridge loading.
- Ensure mirroring control is honored at runtime.
- Keep changes consistent with existing mapper patterns (NROM/UNROM/MMC1/MMC2).

## Affected Code Paths

- Cartridge implementations live in `mmnes_core/src/mapper/`.
- Mapper enum in `mmnes_core/src/mapper.rs`.
- Loader mapper switch in `mmnes_core/src/ines_loader.rs`.
- Cartridge types in `mmnes_core/src/cartridge.rs`.
- Optional: tests in `mmnes_core/src/tests/`.

## Functional Requirements

### 1) New Cartridge: `AxromCartridge`

- File: `mmnes_core/src/mapper/axrom_cartridge.rs`.
- Implements `Memory`, `BusDevice`, and `Cartridge` (same as other mapper modules).
- Uses 32 KB PRG-ROM banks with bank switching.
- Exposes CHR memory (AxROM is CHR-RAM).
- Exposes optional PRG-RAM if present in the iNES header.

### 2) PRG-ROM Bank Switching (CPU $8000-$FFFF)

- PRG-ROM is mapped as a single 32 KB window at `0x8000..=0xFFFF`.
- All CPU writes in the PRG address space update mapper state:
  - Bank select: use low 4 bits (`value & 0x0F`) to choose the 32 KB PRG bank.
  - Mirror select: bit 4 (`value & 0x10`) controls nametable mirroring.
  - Higher bits are ignored.
- Bank selection must be modulo `num_prg_banks` to avoid out-of-range access.
- Reads always come from the current 32 KB bank.

### 3) Mirroring Control (One-Screen)

- AxROM uses one-screen mirroring only.
- When bit 4 of the bank register is:
  - `0` => `PpuNameTableMirroring::SingleScreenLower`
  - `1` => `PpuNameTableMirroring::SingleScreenUpper`
- The mirroring can change at runtime on every write to PRG space.
- The iNES header mirroring bit should only be used as initial state if no writes have occurred; mapper writes override it.

### 4) CHR Memory (PPU $0000-$1FFF)

- AxROM is CHR-RAM only. Prefer allocating CHR-RAM sized from the iNES header:
  - For iNES1 with `chr_rom_size == 0`, loader already reports `chr_ram_size = 8 KB`.
  - For iNES2, use `chr_ram_size` from header.
- If `chr_rom_size > 0`, treat this as unsupported and return `CartridgeError::Unsupported` (AxROM does not use CHR-ROM).
- CHR-RAM is mapped as one 8 KB bank at `PPU_ADDRESS_SPACE`.

### 5) Optional PRG-RAM (CPU $6000-$7FFF)

- If `header.prg_ram_size > 0`, allocate PRG-RAM memory banks at `0x6000..=0x7FFF`:
  - Use 8 KB bank size unless header specifies a different size.
  - Expose via `Cartridge::get_prg_ram()` so the bus maps it.
- If `prg_ram_size == 0`, omit PRG-RAM.

### 6) Loader Wiring

- `mmnes_core/src/mapper.rs`: ensure `NesMapper::AxROM` is exported and its ID/name already exist (mapper ID 7).
- `mmnes_core/src/cartridge.rs`: add `CartridgeType::AXROM` (or `AXROM` naming matching code style).
- `mmnes_core/src/ines_loader.rs`: add match arm to build `AxromCartridge` when mapper is `NesMapper::AxROM`.

### 7) Bus/Memory Expectations

- All `Memory::size()` results must be power-of-two for the `NESBus` masking invariant.
  - 32 KB for PRG-ROM window.
  - 8 KB for PRG-RAM window (if present).
  - 8 KB for CHR-RAM.
- Bus address ranges should follow existing constants:
  - PRG-ROM: `CPU_ADDRESS_SPACE` (0x8000-0xFFFF).
  - PRG-RAM: `0x6000..=0x7FFF`.
  - CHR: `PPU_ADDRESS_SPACE` (0x0000-0x1FFF).

## Implementation Notes

- Follow the pattern used by `UnromCartridge` for banked PRG-ROM and `NromCartridge` for CHR memory wiring.
- Suggested internal fields:
  - `memory_banks: Vec<MemoryBank>`
  - `current_bank: usize`
  - `num_memory_banks: usize`
  - `chr_ram: Rc<RefCell<MemoryBank>>`
  - `prg_ram: Option<Rc<RefCell<MemoryBank>>>`
  - `mirroring: Rc<RefCell<PpuNameTableMirroring>>`
  - `region: Region`
  - `device_type: BusDeviceType`
- Reads should use `current_bank` and address masked to 32 KB (either manually or by letting `MemoryBank` handle it with bus masking).
- Writes in PRG space should update `current_bank` and mirroring only; writes should not modify PRG-ROM.

## Testing Requirements (Minimum)

- Add mapper unit tests in `mmnes_core/src/tests` to validate:
  - Bank selection wraps correctly (`value & 0x0F` then modulo).
  - Mirroring changes between `SingleScreenLower` and `SingleScreenUpper`.
  - CHR-RAM is present and writable.
- If no tests are added, document why in code comments or PR description.

## Non-Goals

- No IRQs or scanline counters (AxROM has none).
- No CHR-ROM banking (AxROM uses CHR-RAM only).
- No battery-backed PRG-RAM persistence (only runtime RAM behavior).
