# REQUIREMENT: Fix VBlank beginning boundary suppression

## Goal
Pass the new boundary test (`TEST_ppu_vblank_begining_1.md`) and the ROM failure:
> "VBlank Beginning: The PPU Register $2002 VBlank flag was not set at the correct PPU cycle."

## Required behavior
When a CPU read of `$2002` occurs on the **same master cycle** that would otherwise set VBlank (scanline 241, dot 1 on NTSC):
1. The read must return VBlank **clear** (bit 7 = 0).
2. The VBlank flag must **remain clear** for the rest of that frame’s dot 1 event (i.e., the set is suppressed).
3. NMI must be **suppressed** for that frame (if enabled).

When the read happens **after** that cycle:
1. The read returns VBlank **set** (bit 7 = 1).
2. The read clears the flag, so subsequent reads return 0.

## Implementation constraints
- Keep cycle-accurate ordering with `NesConsole::step_master_cycle()`; do **not** add instruction-boundary shortcuts.
- Do **not** hack the test or hardcode ROM patterns.
- Avoid breaking existing SingleStepTests and current PPU timing tests.

## Suggested approach (non-prescriptive)
You may choose either of the following, but must satisfy the behavior above:
- **Scheduler split:** advance 1 PPU dot, then perform the CPU bus op, then advance the remaining 2 dots. Use this to detect whether the CPU read coincides with dot 1.
- **PPU-side latch:** track a per-master-cycle flag indicating a `$2002` read occurred, and if the dot-1 event happens during that cycle, suppress VBlank/NMI.

## Acceptance criteria
- The new boundary test passes.
- The original VBlank timing tests (`mmnes_core/src/tests/ppu_2c02.rs`) still pass.
- The ROM no longer reports “VBlank Beginning” failure.
