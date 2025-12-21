Eliminate instruction-boundary legacy execution and converge on a single cycle-accurate execution path
====

# Objective

The emulator has been migrated to a cycle-accurate core.   
However, remnants of the previous instruction-boundary stepping architecture still exist (helpers, scheduling glue, old “fast” pathways, and partially duplicated timing logic).   
These legacy paths create ambiguity, desync risk, and duplicated behavior.   
Claude MUST produce a concrete, low-risk plan to remove all instruction-boundary execution code and leave exactly one execution path: cycle-accurate.


# Hard constraints (non-negotiable)

1. After the refactor, there MUST be one and only one authoritative execution mechanism for CPU/APU/PPU/bus: a per-CPU-cycle tick pipeline.
2. There MUST be no instruction-step entrypoints (public or internal), no instruction-based “catch-up”, no “run N instructions” loops, no “execute instruction then sync clocks”, and no alternate scheduling path that bypasses the cycle tick.
3. Timing, DMA, IRQ polling, open-bus behavior, APU frame sequencing, and PPU dot timing MUST all be driven exclusively from the cycle-accurate path.
4. The plan MUST include how to verify equivalence of visible behavior (games/tests), and MUST include mechanical steps for identifying and deleting dead/legacy code without breaking the cycle-accurate pipeline.

# Required deliverable from Claude

Claude MUST output a step-by-step migration plan that includes:

## Architecture inventory (must be explicit)

1. Identify all current execution entrypoints and categorize them as:
  - Cycle-accurate tick path
  - Instruction-boundary path 
  - Hybrid/bridge path (instruction stepping that calls multiple cycle ticks or vice versa)

2. Provide a list of all functions/modules that directly or indirectly:
  - execute a full instruction at once,
  - schedule PPU/APU work per instruction,
  - accumulate cycles in counters and then “flush” clocks,
  - have names like step_instruction, run_instruction, exec_opcode, clock_to, sync, catch_up, run_cycles_from_instruction, etc.

3. For each legacy element found, specify:
  - where it is called from,
  - why it exists,
  - what the cycle-accurate replacement is (or will be).

## Single-source-of-truth execution pipeline (must be defined)

Claude MUST define the final pipeline as a single function that everything funnels through, conceptually like:

`Machine::tick_cpu_cycle() (or equivalent)`

and must state exactly what happens in what order within one CPU cycle, at minimum:

Determine CPU bus intent for this cycle (read/write, address, data_out if write).

1. DMA arbitration and RDY stall decision.
2. Execute exactly one bus action (CPU or DMA master).
3. Advance CPU internal microstate by exactly one cycle (or hold if RDY).
4. Tick APU and PPU by the correct ratios for exactly one CPU cycle.
5. Update IRQ/NMI line sampling at cycle-accurate points (no instruction-boundary polling).

This is not a suggestion; Claude MUST describe a deterministic per-cycle ordering.

## Removal strategy (the meat)

Claude MUST propose a plan that removes legacy execution without introducing regressions.   
The plan MUST:

1. Introduce a compilation-time “ban” on instruction stepping:
 - e.g., delete functions or gate them behind #[cfg(feature = "legacy")] initially and then remove the feature at the end. 
 - no runtime flags that choose between paths.

2. Replace all callers of instruction-step APIs with the cycle tick:
  - UI frame loop 
  - audio callback stepping 
  - debugger stepping 
  - test harness stepping 
  - rewind/movie tools (if any)

3. Unify timekeeping:
  - remove any code that converts “instructions → cycles” as a scheduling primitive 
  - remove any “cycle accumulator flushed per instruction” 
  - enforce that only cycle ticks advance clocks

4. Collapse duplicate logic:
  - if both paths contain versions of DMA, interrupts, open bus, or PPU stepping, legacy copies MUST be deleted and any missing functionality moved into the cycle path only.

5. Delete or rewrite adapters:
- any wrapper that “runs 1 instruction by ticking cycles until opcode completes” MUST either:
   - become a debugger-only helper that calls cycle ticks but does NOT reintroduce instruction scheduling, OR
   - be removed if it risks creating a second execution path.

Important: “debugger step instruction” is allowed only if it is implemented as:

“call tick_cycle repeatedly until the CPU reports instruction boundary,”
and it MUST NOT have its own clocking model. It must be a thin loop that delegates to the single tick.

## Test/verification plan (required)

Claude MUST propose how to prove the legacy path is gone and behavior remains correct:


1. Static verification
  - Grep-based checks for banned symbols (step_instruction, run_instruction, etc.)
  - Module ownership rules (e.g., only machine/tick.rs can call apu.tick_* / ppu.tick_*)
  - Ensure there is exactly one “top-level clock advancement” function


2. Runtime invariants / assertions
  - Add debug-only counters that assert:
    - PPU dots advance exactly 3 per CPU cycle (NTSC)
    - APU and CPU cycle counters maintain expected relationships
    - DMA stall cycles match expected counts on known triggers
  - Add assertions that no code path advances CPU time without calling the cycle tick

3. Behavior tests
  - Run CPU/PPU/APU timing test ROMs (whatever the project already uses)
  - Add at least one “golden trace” test:
    - record a short window of per-cycle bus transactions from a known ROM
    - compare against a committed expected trace for determinism

Claude MUST list concrete tests and what signals they validate (bus log, IRQ timing, DMA stall length, etc.), not just “run tests.”

## Risk management and sequencing (required)

Claude MUST present the plan in an order that minimizes breakage:

1. First: make cycle tick the only scheduler used by frontends/tests (even if legacy code still exists).
2. Second: delete/disable legacy entrypoints and fix compiler errors by routing all time advancement through the tick.
3. Third: remove redundant timing logic and counters that were only needed for instruction stepping.
4. Fourth: tighten invariants, remove feature flags, delete dead modules.

Claude MUST include “rollback points” (git commits or checkpoints) at which the code compiles and tests pass.


# Definition of done

The task is complete only when:
1. There is exactly one execution path that advances emulation time: the per-cycle tick.
2. No instruction-step timing model exists anywhere (including hidden helpers).
3. All frontends/debugger/tests use the cycle tick as the sole source of time.
4. The emulator passes the existing test suite and the additional invariants/traces described above.


# Tone requirement to Claude

Claude MUST be ruthless about deleting code and eliminating ambiguity.   
If something can accidentally create a second scheduling path, it MUST be removed or rewritten to delegate to the cycle tick.