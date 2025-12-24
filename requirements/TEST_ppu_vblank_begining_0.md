# TEST: PPU VBlank Beginning (CPU-visible timing)

## Goal
Reproduce the ROM failure: "VBlank Beginning - The PPU Register $2002 VBlank flag was not set at the correct PPU cycle."

The test must prove whether the CPU can observe VBlank (PPUSTATUS bit 7) on the *same master cycle* that the PPU reaches scanline 241, dot 1 (NTSC).

## Scope
- This is a scheduler/ordering test that must use `NesConsole::step_master_cycle()` so it exercises the real CPU/PPU/APU ordering.
- The test should be deterministic and not depend on external ROMs.

## Test design
### 1) Test harness requirements
- Build a minimal console with a valid iNES ROM in a temp file:
  - NROM-128 header (16 KB PRG, 8 KB CHR).
  - PRG/CHR contents can be all zeros.
- Use the console builder so the PPU and bus mapping are real.

### 2) CPU behavior for the test
- Inject a test CPU that performs a read of `$2002` every master cycle and records the returned value.
- The CPU must implement the `CPU` trait and expose a simple `last_read()` getter for assertions.
- If a helper is needed, add a **test-only** builder path, e.g. `NesConsoleBuilder::with_cpu_instance(...)` under `#[cfg(test)]`, or a `pub(crate)` test constructor for `NesConsole` so tests can inject the CPU.

### 3) Execution steps
- Reset the console, then step master cycles until the PPU transitions to **scanline 241, dot 1**.
  - Use `PPU::get_scanline()` / `PPU::get_dot()` or equivalent public getters on the PPU instance inside the console.
  - Track the cycle that produces that transition (i.e., the cycle where the *post-step* scanline/dot is 241/1).
- On that exact master cycle, capture the CPU’s `$2002` read value.

## Assertions (must be in the test)
1. The cycle immediately **before** the transition to scanline 241, dot 1 returns `$2002` with VBlank **clear** (bit 7 = 0).
2. The cycle that produces scanline 241, dot 1 returns `$2002` with VBlank **set** (bit 7 = 1).

## Expected outcome
- The test should fail on current code (because CPU reads occur before PPU dot advancement).
- After fixing the scheduler/ordering, the test must pass without changing the assertions.

## Notes
- Keep the test small and self-contained under `mmnes_core/src/tests/`.
- Do not add external test ROMs to the repo; generate the minimal iNES file in the test.
- Any test-only helper APIs must be `#[cfg(test)]` or `pub(crate)` to avoid public API changes.
