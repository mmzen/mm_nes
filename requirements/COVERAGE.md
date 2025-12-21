# Requirement: Reach ≥ 80% Test Coverage on `mmnes_core`

## Objective
Increase automated test coverage for the `mmnes_core` Rust crate/module to **at least 80% line coverage**, measured from a clean build using a reproducible, documented command. The goal is **meaningful coverage** that exercises real behavior (cycle timing, bus effects, DMA, PPU/APU interactions), not superficial “touch the line” tests.

## Scope
This requirement applies to **all code within `mmnes_core`**, including (but not limited to):

- CPU core execution pipeline (cycle-accurate)
- Bus arbitration and open-bus behavior
- DMA (OAM DMA + DMC DMA) timing and overlap rules
- APU register behavior and DMC fetch behavior
- PPU register interface behavior relevant to `mmnes_core`
- Mapper interfaces and core memory map logic (if implemented inside `mmnes_core`)
- Any public API surface and internal helpers that affect correctness

Excluded only if explicitly justified by Claude with a technical reason and an alternative validation strategy (e.g., generated code, platform glue, or code that is unreachable by construction).

## Coverage Target
- Achieve **≥ 80% line coverage** on `mmnes_core`.
- The coverage report MUST be generated in CI-friendly form (e.g., text summary + HTML artifact).
- Coverage MUST be computed for the `mmnes_core` crate/module only (not the workspace average unless it also proves `mmnes_core` ≥ 80%).

## Measurement Tooling (must be implemented)
Claude MUST implement and document one of the following coverage paths (choose what fits the repo best, but it must be stable and reproducible):

### Option A (preferred): `cargo llvm-cov`
- Add and configure `cargo llvm-cov` usage so developers can run:
  - `cargo llvm-cov -p mmnes_core --lcov --output-path lcov.info`
  - `cargo llvm-cov -p mmnes_core --html`
  - `cargo llvm-cov -p mmnes_core --summary-only`

### Option B: `tarpaulin`
- Add and configure `cargo tarpaulin` (only if LLVM-cov is not viable on the project setup).

The chosen tool and exact commands MUST be added to the repository documentation (e.g., `CONTRIBUTING.md` or `README.md`) and used in CI.

## Test Strategy Requirements (no low-effort coverage)
Claude MUST create tests that are behavior-driven and deterministic. At minimum, the test suite must include all of the following categories:

### 1) Unit tests for pure logic
- Address decode logic
- Register side-effect helpers
- Mapper bank selection logic (if present)
- APU/PPU register read/write semantics that are pure or mockable

### 2) Deterministic integration tests with bus-level traces
Add tests that run the machine for a fixed number of CPU cycles and assert:
- Exact bus operations (address, R/W, data) per cycle for a short window
- Exact stall lengths for DMA sequences
- Correct priority and overlap behavior (DMC steals GET from OAM DMA)

At minimum, include:
- OAM DMA 513-cycle case
- OAM DMA 514-cycle case (alignment)
- DMC DMA 3-cycle case
- DMC DMA 4-cycle case (alignment)
- DMC DMA overlapping OAM DMA causing OAM realignment

### 3) Invariant/property tests (only where deterministic)
Where appropriate, add property tests (e.g., `proptest`) for invariants such as:
- PPU dot count advances at the correct ratio relative to CPU cycles (if clocking is inside `mmnes_core`)
- `open_bus` updates only when external bus is driven
- DMA cannot successfully halt on CPU write cycles

Property tests MUST be constrained to deterministic behavior and MUST NOT introduce flaky randomness.

### 4) Regression tests for previously fixed bugs
If there are known issues (timing bugs, controller glitches, PPUDATA increment issues under DMA stalls, etc.), add targeted regression tests that:
- reproduce the bug on the old behavior
- prove correctness on the new behavior

## Determinism Requirements (critical)
- Tests MUST be deterministic across runs and platforms.
- Any “random power-on alignment” or “decay randomness” behavior MUST be made testable via:
  - injectable PRNG seed, or
  - deterministic test mode, or
  - explicit initialization inputs
- Tests MUST NOT rely on wall-clock time, audio callbacks, threads, or non-deterministic scheduling.

## Required Refactors to Enable Coverage (allowed and expected)
Claude is permitted (and expected) to refactor `mmnes_core` to make it testable, with the following constraints:

- No second execution path may be introduced (cycle-accurate tick remains the only time-advancement mechanism).
- Introduce small interfaces to enable mocking:
  - `Bus` trait or equivalent
  - deterministic test bus that records per-cycle accesses
  - hooks to step exactly N CPU cycles

Any refactor MUST preserve public API behavior unless the change is explicitly justified and documented.


## Deliverables
Claude MUST produce:

1. A coverage command section in repo docs with exact commands.
2. New/updated tests that drive coverage to ≥ 80% on `mmnes_core`.
3. A bus-trace test harness (if not already present) suitable for asserting per-cycle behavior.
4. A CI job that enforces the coverage threshold.

## Definition of Done
This requirement is satisfied only when:

- Running the documented command locally produces **≥ 80% line coverage for `mmnes_core`**.
- CI enforces the same threshold and passes.
- The added tests meaningfully validate correctness (timing, bus behavior, DMA sequencing), not just execute code paths.
