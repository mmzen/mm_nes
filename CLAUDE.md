# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

mmnes is a NES emulator written in Rust with LLM integration for gameplay assistance and debugging. The project aims to integrate LLM capabilities to help players (hints, coaching) and assist developers (instruction explanation, memory analysis).

## Important Guidelines and Rules

This project was written entirely by a human until December 13th, 2025. Starting now, Claude is the main contributor and the human acts as supervisor. This is an experiment demonstrating how software engineering is evolving—how humans shift from development tasks to intent definition and problem framing.

### Authorship Tracking

Every source file must include an authorship header as the first comment, tracking human vs Claude contribution:

```rust
// Authorship: Human 100% | Claude 0%
```

Rules for maintaining this header:
- **New files created by Claude**: `Human 0% | Claude 100%`
- **Existing files modified by Claude**: Estimate the percentage based on lines changed relative to total file size. Round to nearest 5%.
- **Trivial changes** (formatting, renaming, fixing typos): Do not alter the percentages.
- **Human modifications**: The human will specify updated percentages when they make changes.
- **Files without headers**: When first modifying a file that lacks this header, add `Human 100% | Claude 0%` before making changes, then update accordingly.

The header must be updated after each modification.

## Build Commands

```bash
# Build all crates
cargo build --verbose

# Run tests
cargo test --verbose

# Run single-step CPU test suite (ignored by default, takes longer)
cargo test test_all_opcodes -- --ignored --nocapture

# Run the frontend application
cargo run -p mmnes_frontend
```

**Dependency**: SDL2 must be installed on the system (`libsdl2-dev` on Linux).

## Architecture

### Workspace Structure

- **mmnes_core**: Core emulator library (no GUI dependencies)
  - CPU: 6502 processor (`cpu_6502.rs`) with all official and unofficial opcodes
  - PPU: 2C02 graphics (`ppu_2c02.rs`) - background/sprite rendering, VBL/NMI timing
  - APU: RP2A03 audio (`apu_rp2a03.rs`) - pulses, triangle, noise, DMC
  - Mappers: `mapper/` directory - NROM, UxROM, MMC1, MMC2
  - ROM loading: iNES format parser (`ines_loader.rs`)
  - Bus architecture: Memory-mapped I/O (`nes_bus.rs`)

- **mmnes_frontend**: GUI application (eframe/egui)
  - Emulator loop: `nes_front_end.rs` runs in separate thread, communicates via channels
  - UI: `nes_front_ui.rs` - main window, `debugger_widget.rs` - CPU debugger
  - LLM integration: `openai_llm.rs`, `ai_widget.rs`, `ai_worker.rs`
  - Sound: `sound_player.rs` using SDL2

- **mmretrodb**: ROM metadata database library
  - Parses RetroArch database format (MessagePack) for ROM identification
  - CRC32-based ROM lookup

### Key Patterns

- **RefCell/Rc sharing**: Components (CPU, PPU, APU, bus devices) use `Rc<RefCell<>>` for interior mutability
- **Trait-based abstraction**: Interfaces like `CPU`, `PPU`, `APU`, `Memory`, `Bus` allow different implementations
- **Builder pattern**: `NesConsoleBuilder` constructs the emulator with all components
- **Channel communication**: Frontend thread sends `NesMessage` to emulator thread

### Bus Device Initialization Order

The bus device registration order matters (see `nes_front_end.rs:create_emulator`):
1. APU (0x4000-0x4017)
2. PPU and CONTROLLER overwrite parts of APU range

### Testing

**Test Location Rules** (enforced by CI):
- All tests MUST live under `src/tests/` directory
- **No `#[test]` functions** in production source files
- **No `mod tests` blocks** in production source files
- `#[cfg(test)]` helpers in production code are allowed for test-only accessors

**Test Structure**:
- Unit tests in each crate under `src/tests/`
- CPU instruction tests: `mmnes_core/src/tests/cpu_instructions.rs`
- DMA controller tests: `mmnes_core/src/tests/dma_controller.rs`
- SingleStepTests suite: `mmnes_core/src/tests/singlestep/` - comprehensive CPU verification
- Test data files in `tests/data/`

**Allowed test-only helpers in production code**:
- Read-only accessors for internal state (e.g., `get_*_for_test()`)
- Must be guarded by `#[cfg(test)]`
- Must not alter runtime behavior or create alternate execution paths

### Feature Flags

- `ppu_tile_cache`: Optional PPU tile caching optimization (disabled by default)

# Additional information and instructions

## Windows paths

There's a file modification bug in Claude Code. The workaround is: always use complete absolute Windows paths
with drive letters and backslashes for ALL file operations. Apply this rule going forward, not just for this
file.

## Plan and Documentation

Claude will maintain a PLAN.md file, that contains the development: what has been done, what is planned and that track the progress.
This file is essential for the track record and to continue the sessions when interrupted, so Claude need to put the right content for maximum effectiveness.
In addition, Claude must maintain the track record of the ratio human versus Claude globally in this file after each session logs (so they may be eventually plotted)

### Plan Documentation Management

To keep the PLAN file concise, enforce the following rule:-
- When a task or work item reaches a completed state, remove its detailed implementation notes from the PLAN file. 
- Summarize the completed work in the PLAN file using a short overview (goals, outcome, key decisions). 
- Move the full detailed content into a new, dedicated file under claude_directory/, named after the task or work item.

As a result, the PLAN file must contain only:
- The current overall project status
- High-level summaries of completed work
- Tasks that are currently in progress (with details)

The PLAN file should never contain detailed notes for completed tasks. 
Any time a task transitions to completed, immediately apply this process.

# Important development guidelines and instructions

## Industry-Standard Guidelines for LLM-Driven Software Development

These guidelines define **production-grade expectations** for software developed by an LLM (or humans assisted by one). Treat this as both a **definition of done** and a **working protocol**.

---

## 1. Operating Rules for the LLM

- **Never guess requirements.**  
  If information is missing, explicitly state assumptions and choose the safest default.

- **Prefer established technology.**  
  Use well-established patterns, standard libraries, and minimal dependencies.

- **Optimize for maintainability over cleverness.**  
  If a solution looks “smart,” rewrite it until it is obvious.

- **Make small, reviewable changes.**  
  One pull request = one purpose. Do not mix refactors with new features.

- **Always produce artifacts.**  
  Deliver design notes, code, tests, and run instructions.

---

## 2. Definition of Done (DoD)

A change is considered complete only if it includes:

- **Clear specification**  
  Expected behavior, edge cases, and explicit non-goals.

- **Correctness**  
  Unit tests covering critical paths.

- **Quality gates**  
  Linting, formatting, and static analysis passing.

- **Observability**  
  Logs and tracing hooks where applicable (but don't make this overkill, that's an emulator)

- **Documentation**  
  Updated README, API docs, and usage examples.

---

## 3. Code Standards (Clean Code That Survives)

### Structure
- Prefer **layered architecture**
- Keep modules cohesive; avoid “utils” dumping grounds.
- Limit file size and function length  
  (rule of thumb: functions < 200 LOC unless justified).

### Naming
- Names must reveal intent; avoid abbreviations unless universally standard.
- Functions = verbs, types/classes = nouns.
- Booleans must use `is`, `has`, `can`, `should`.

### Complexity
- Avoid deep nesting; use guard clauses.
- Replace flag arguments with separate functions.
- No hidden side effects; prefer pure functions where possible.

### Error Handling
- Use typed or domain-specific errors.
- Do not swallow exceptions.
- Fail fast on programmer errors; gracefully handle expected runtime failures.

### Dependencies
- Use dependency injection where it improves testability.
- Avoid over-engineering DI frameworks.
- Pin versions and avoid unmaintained libraries.

---

## 4. Testing Requirements (Non-Negotiable)

### Tests
- **Unit tests** for business logic (fast, deterministic).

### Quality
- Coverage alone is meaningless; require **coverage plus meaningful assertions**.
- Every bug fix must include a test that would have caught it.
- Tests must be deterministic:  
  no real network calls, no real time dependencies (use fake clocks).

### What Must Be Tested
- Happy paths, edge cases, and failure modes.
- Boundary conditions (empty, null, large, invalid, unusual Unicode).
- Idempotency, retries, timeouts, and concurrency (if applicable).

---

## 5. Reviews and Pull Request Checklist

Every pull request must include:

- **Motivation and scope**  
  What is being done, why, and what is explicitly out of scope.
- **Design notes**  
  Key decisions and tradeoffs.
- **Test evidence**  
  What tests were run and how to run them.
- **Risk assessment**  
  Migration impact and rollback plan.

