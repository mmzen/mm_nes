# Requirement: Tests Live Exclusively in `src/tests/`, with Limited `#[cfg(test)]` Helpers Allowed

## Objective
All **test cases** MUST live in the `src/tests/` directory and be executed as **integration-style tests**.

Production source files under `src/` MUST remain free of inline tests and test modules.

However, **test-only helper APIs** inside production modules are allowed *solely* to support tests, under strict constraints.

---

## Hard Rules (Non-Negotiable)

### 1) Location of tests
1. **No test cases may exist directly inside production source files under `src/`** (e.g. `src/cpu.rs`, `src/bus.rs`, etc.).
2. **No `#[test]` functions may appear in production source files under `src/`.**
3. **No `mod tests { ... }` blocks are allowed in production source files under `src/`.**
4. **All test cases MUST live under the directory: `src/tests/`**

This directory is the **single source of truth** for all test code.

---

### 2) Allowed test-only helpers in production code
The following is explicitly allowed inside production modules under `src/`:

- Functions, methods, trait implementations, or accessors that exist **only to support tests**, provided that **all** of the following are true:
1. They are guarded by `#[cfg(test)]` (or an equivalent test-only configuration).
2. They expose **only the minimum surface area required** for tests.
3. They do **not** alter runtime behavior, timing, or side effects.
4. They do **not** introduce an alternate execution path.

These helpers exist strictly to allow tests in `src/tests/` to observe or access internal state.

#### Examples of allowed helpers
- Read-only accessors for internal state
- Narrow getters for private fields
- Test-only constructors or reset hooks
- Test-only introspection of counters, latches, or internal flags

#### Examples of disallowed helpers
- Test-only logic that changes timing, ordering, or scheduling
- Test-only “fast paths” or shortcuts
- Test-only execution or stepping mechanisms
- Test-only behavior toggles that affect emulator semantics

---

### 3) Visibility discipline
- Claude MUST NOT make large subsystems `pub` or `pub(crate)` solely to satisfy tests.
- If tests require access to internals, Claude MUST:
- Prefer narrow `#[cfg(test)]` accessors, OR
- Introduce a small test-only trait or adapter gated behind `#[cfg(test)]`.
- Any test-only API MUST be clearly named to indicate its purpose, e.g.:
- `*_for_test`
- `*_test_only`

---

### 4) Explicitly forbidden
The following are forbidden without exception:

- Any `#[test]` attribute in production source files under `src/`
- Any `mod tests` block in production source files under `src/`
- Any test logic embedded in production execution paths
- Any duplication of execution paths for tests versus production
- Any test-only code that changes observable emulator behavior

---

## Required Actions

### 1) Audit and cleanup
Claude MUST:

- Search the entire `src/` tree for:
- `#[test]`
- `mod tests`
- Remove all such occurrences from production source files
- Ensure all test cases are implemented under `src/tests/`

---

### 2) Introduce test-only helpers where required
For each test that previously depended on private internals:

- Introduce the **smallest possible** `#[cfg(test)]` helper in the relevant production module
- Add a short comment explaining:
- which test(s) rely on it
- why the helper exists

---

### 3) Automatic enforcement (mandatory)
Claude MUST add an automated enforcement mechanism.

#### Required CI check
CI MUST fail if any of the following appear in production source files under `src/`:

- `#[test]`
- `mod tests`

The check MUST:
- Ignore files under `src/tests/`
- Allow `#[cfg(test)]` helpers in production code

---

### 4) Documentation update
Claude MUST update project documentation to clearly state:

- All tests live under `src/tests/`
- Inline tests inside production modules are forbidden
- `#[cfg(test)]` helpers are allowed but must be minimal and non-behavioral

---

## Definition of Done
This requirement is satisfied only when:

1. All test cases live under `src/tests/`.
2. Production source files under `src/` contain **zero** `#[test]` functions and **zero** `mod tests`.
3. Any `#[cfg(test)]` code in production modules is strictly limited to helpers/accessors.
4. CI fails automatically if a test is reintroduced into a production module.
5. All tests pass using only the `src/tests/` layout.

Any deviation from this structure is a failure.