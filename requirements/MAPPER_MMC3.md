# Requirement: Implement MMC3 (Mapper 4) in Rust NES Emulator

## Objective
Implement **MMC3 (Nintendo MMC3 / Mapper 4)** in the cartridge/mapper subsystem such that **commercial MMC3 games and standard test ROMs boot and behave correctly**, including:
- Correct **PRG/CHR bank switching**
- Correct **nametable mirroring control**
- Correct **PRG-RAM enable/write-protect**
- Correct **scanline IRQ counter behavior** (A12 edge-based)

This task is complete only when all Acceptance Criteria pass.

---

## Scope
### In scope
- iNES Mapper **4** (MMC3)
- PRG ROM banking (8KB units)
- CHR ROM banking (1KB/2KB units) and CHR RAM support
- PPU nametable mirroring control (vertical/horizontal)
- PRG-RAM enable + write protect
- IRQ latch/reload/enable/disable using PPU A12 rising edges
- Reset behavior (power-on and soft reset consistent with common MMC3 expectations)

### Out of scope (explicit non-goals for this task)
- MMC6 differences, MMC3A vs MMC3B quirks beyond the baseline described here
- 4-screen mirroring (handled by iNES header/board config; MMC3 mirroring writes should be ignored if hardwired)
- Bus conflict emulation (unless your emulator already models it globally)
- Save-state system (unless already present)

---

## Interfaces & Integration Points
### Required integration
- Mapper must hook into:
  - CPU memory map: $6000–$7FFF (PRG-RAM), $8000–$FFFF (MMC3 registers + PRG-ROM)
  - PPU CHR reads: $0000–$1FFF (CHR banking)
  - PPU nametable mirroring selection (if cartridge not fixed)
  - CPU IRQ line: assert/clear mapper IRQ based on IRQ logic

### Required APIs (adapt to your codebase naming, but functionality must exist)
- `mapper.cpu_read(addr) -> u8`
- `mapper.cpu_write(addr, data)`
- `mapper.ppu_read(addr) -> u8` (or `chr_read`)
- `mapper.ppu_write(addr, data)` for CHR RAM (writes ignored if CHR ROM)
- `mapper.on_ppu_address(addr)` OR `mapper.ppu_a12_edge(rising: bool)` to drive A12 edge detection
- `mapper.poll_irq() -> bool` or direct `cpu.set_irq_line(level)` integration

---

## MMC3 Register Map (CPU writes)
All addresses are in CPU space; mirroring of even/odd handled by `addr & 0xE001`.

### Bank select: $8000 (even)
- Bits 0–2: `bank_register_index` (0..7)
- Bit 6: `prg_mode` (0/1)
- Bit 7: `chr_mode` (0/1)

### Bank data: $8001 (odd)
Write value to the currently selected bank register `R[bank_register_index]`.

Bank registers semantics:
- R0, R1: 2KB CHR banks (must be even-numbered; low bit ignored)
- R2..R5: 1KB CHR banks
- R6, R7: 8KB PRG banks

### Mirroring: $A000 (even)
- Bit 0:
  - 0 = vertical mirroring
  - 1 = horizontal mirroring
- If cartridge is fixed mirroring from header/board, this write must be ignored.

### PRG-RAM protect: $A001 (odd)
- Bit 7: PRG-RAM enable (1 = enable reads/writes)
- Bit 6: PRG-RAM write protect (1 = write protect)
- If PRG-RAM disabled: reads should typically return open bus or 0xFF per emulator policy; choose one policy and document it.

### IRQ latch: $C000 (even)
- Set `irq_latch = data`

### IRQ reload: $C001 (odd)
- Set `irq_reload = true` (do not directly load counter here; it affects next A12 edge behavior)

### IRQ disable: $E000 (even)
- `irq_enabled = false`
- Clear IRQ pending/line immediately

### IRQ enable: $E001 (odd)
- `irq_enabled = true`

---

## PRG Banking Rules
MMC3 maps CPU $8000–$FFFF as four 8KB slots:
- $8000–$9FFF: slot 0
- $A000–$BFFF: slot 1
- $C000–$DFFF: slot 2
- $E000–$FFFF: slot 3

Fixed banks:
- The **last** 8KB bank is fixed to $E000–$FFFF
- The **second-to-last** 8KB bank is fixed to either $8000–$9FFF or $C000–$DFFF depending on `prg_mode`

Rules:
- If `prg_mode = 0`:
  - $8000 = bank R6
  - $A000 = bank R7
  - $C000 = fixed second-to-last
  - $E000 = fixed last
- If `prg_mode = 1`:
  - $8000 = fixed second-to-last
  - $A000 = bank R7
  - $C000 = bank R6
  - $E000 = fixed last

Bank index resolution:
- PRG bank number selects an 8KB bank within PRG ROM.
- Masking: bank index must wrap within available banks (e.g., `bank % prg_8k_banks`).

---

## CHR Banking Rules
MMC3 maps PPU $0000–$1FFF as eight 1KB slots.

Two modes controlled by `chr_mode`:
- If `chr_mode = 0` (normal):
  - $0000–$07FF = R0 (2KB, even-aligned)
  - $0800–$0FFF = R1 (2KB, even-aligned)
  - $1000–$13FF = R2 (1KB)
  - $1400–$17FF = R3 (1KB)
  - $1800–$1BFF = R4 (1KB)
  - $1C00–$1FFF = R5 (1KB)
- If `chr_mode = 1` (swapped):
  - $0000–$03FF = R2
  - $0400–$07FF = R3
  - $0800–$0BFF = R4
  - $0C00–$0FFF = R5
  - $1000–$17FF = R0 (2KB, even-aligned)
  - $1800–$1FFF = R1 (2KB, even-aligned)

Even alignment requirement:
- For R0/R1, ignore bit 0 (force even): `bank &= 0xFE`

CHR ROM vs CHR RAM:
- If CHR ROM exists: PPU writes to $0000–$1FFF are ignored.
- If CHR RAM: reads/writes go to RAM with the same bank mapping.

---

## IRQ Counter Behavior (A12 edge-based)
### A12 rising edge detection
- MMC3 clocks the IRQ counter on **rising edges of PPU A12** ($1000 bit) with a **minimum low-time filter**.
- Implement a practical filter:
  - Track last A12 state.
  - Only count a rising edge if A12 was low for at least **N PPU cycles** (commonly 2–8). Use **8 PPU cycles** as default unless your emulator already has a known-good approach.
  - The goal: ignore rapid A12 toggles during CHR fetches that are too close.

### Counter update on a qualified A12 rising edge
On each qualified A12 rising edge:
1. If `irq_counter == 0` OR `irq_reload == true`:
   - `irq_counter = irq_latch`
   - `irq_reload = false`
2. Else:
   - `irq_counter -= 1`
3. If after step (1/2) the counter becomes **0** AND `irq_enabled == true`:
   - assert IRQ pending/line

### IRQ line handling
- `IRQ disable ($E000)` must:
  - clear `irq_enabled`
  - clear IRQ pending/line immediately
- `IRQ enable ($E001)` enables future IRQs; it does not retroactively fire.

---

## Reset Behavior
On power-on:
- Initialize bank registers and modes to a known baseline:
  - `bank_register_index = 0`
  - `prg_mode = 0`
  - `chr_mode = 0`
  - bank registers R0..R7 = 0
  - `irq_latch = 0`
  - `irq_counter = 0`
  - `irq_reload = false`
  - `irq_enabled = false`
  - IRQ line cleared
- PRG mapping must still ensure fixed last bank at $E000 and fixed second-to-last per prg_mode.

On soft reset:
- Keep PRG/CHR banks as per common emulator behavior unless your emulator has a strict reset policy; document your choice and keep it deterministic.

---

## Implementation Constraints
- Must not change CPU or PPU core semantics except adding required mapper hooks.
- Must keep “one bus op per CPU cycle” invariant if your DMA arbiter depends on it.
- Avoid allocations during per-read/per-write hot paths.

---

## Verification & Acceptance Criteria
### Required automated checks
1. **Deterministic bank mapping tests**
   - Unit/integration tests that write to $8000/$8001 and verify PRG/CHR bank indices resolve correctly in all combinations of `prg_mode` and `chr_mode`.

2. **PRG-RAM protect tests**
   - When disabled: writes do not modify RAM.
   - When enabled + write-protect: reads work, writes blocked.
   - When enabled + not write-protect: reads/writes work.

3. **Mirroring control tests**
   - $A000 bit0 toggles vertical/horizontal mirroring (unless fixed).

4. **IRQ tests (must exist)**
   - A12 edge detection produces counter clocks only on qualified rising edges.
   - Reload behavior matches spec: reload flag affects next clock.
   - IRQ enable/disable behavior correct (disable clears line immediately).

### Required ROM-based functional tests
Run a known MMC3 test suite and at least 2 MMC3 games.
- Test suite must include IRQ behavior validation (scanline IRQ correctness).
- Collect evidence:
  - “All tests passed” output or screenshots/logs
  - A per-frame checksum match vs reference emulator for at least one game scene (optional but recommended)

**Acceptance Criteria (binary)**
- MMC3 games boot and are playable without obvious graphics/IRQ glitches.
- MMC3 IRQ test ROM(s) pass.
- No assertion failures in your scheduler/bus arbiter during MMC3 gameplay.

---

## Deliverables
- Mapper implementation code (MMC3 / Mapper 4)
- Tests for bank mapping, mirroring, PRG-RAM protect, IRQ counter
- Short developer doc: “MMC3 behavior implemented, known limitations, filter constant chosen”

