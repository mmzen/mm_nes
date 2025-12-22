INTERRUPT FLAG LATENCY
---

Below is a **precise, non-negotiable requirement** you can hand to Claude.
It is written as a **specification**, not advice, and is tightly scoped to the **current CPU implementation you provided**.

---

# Requirement: Correct IRQ Entry at Instruction Boundary (DMC IRQ Latency)

## Context

This requirement applies to the provided `Cpu6502` implementation using a **cycle-stepped state machine** with explicit states:

* `FetchOpcode`
* `Executing`
* `InterruptSequence`

The emulator currently fails the **AccuracyCoins – Interrupt Flag Latency (error code 1)** test, meaning the CPU does not always enter the IRQ sequence at the correct time when a **DMC IRQ** is asserted.

---

## Objective

Ensure that **when a DMC IRQ is asserted and the CPU Interrupt Disable (I) flag is clear**, the CPU **enters the IRQ interrupt sequence immediately at the next instruction boundary**, without executing an additional opcode.

---

## Mandatory Behavioral Rules

### 1. IRQ Latching (Polling Phase)

The CPU **must latch IRQ independently of the I flag**.

Specifically:

* During `poll_interrupts()`:

    * If the IRQ line is asserted, `latched_irq` **must be set to true**
    * The I flag **must NOT affect IRQ latching**
* The I flag is evaluated **only** when deciding whether to service the interrupt

This ensures IRQs are not lost when:

* IRQ is asserted while `I = 1`
* `I` is cleared shortly before the instruction boundary

---

### 2. FetchOpcode Interrupt Preemption (Critical)

In `CpuCycleState::FetchOpcode`, the CPU **must check for a pending interrupt before fetching an opcode**.

Required behavior:

1. At the **start** of `FetchOpcode`:

    * Clear `latched_irq` / `latched_nmi`
    * Call `poll_interrupts()`
2. Immediately call `check_interrupt_from_latch()`
3. If an interrupt is pending:

    * **Do not fetch or decode an opcode**
    * **Do not advance PC**
    * Perform a **dummy read at the current PC**
    * Transition directly to:

      ```rust
      CpuCycleState::InterruptSequence {
          interrupt_type,
          cycle: 2,
          state: InstructionState::default()
      }
      ```
    * Count this dummy read as **interrupt cycle 1**

This behavior is mandatory and must take priority over opcode fetch.

---

### 3. No Extra Instruction Execution

Once an IRQ is pending at an instruction boundary:

* The CPU **must not execute any part of the next instruction**
* No opcode fetch, decode, or operand fetch may occur
* The next bus activity must belong to the interrupt sequence

Executing even a single opcode before servicing the IRQ is a **hard failure**.

---

### 4. InterruptSequence Semantics

The IRQ interrupt sequence must follow **exact 6502 timing**:

| Cycle | Operation                  |
| ----: | -------------------------- |
|     1 | Dummy read at current PC   |
|     2 | Dummy read at current PC   |
|     3 | Push PCH                   |
|     4 | Push PCL                   |
|     5 | Push P (B=0, unused=1)     |
|     6 | Read IRQ vector low, set I |
|     7 | Read IRQ vector high, jump |

The sequence must be uninterrupted and must not depend on opcode-related state.

---

### 5. DMC IRQ Persistence

The CPU implementation **must assume**:

* DMC IRQ is **level-triggered**
* Once asserted, it remains asserted until explicitly cleared
* The CPU must therefore be able to observe the IRQ across multiple cycles and instruction boundaries

No edge-triggered or single-cycle IRQ behavior is permitted.

---

### 6. DMA / Halt Interaction (Non-Negotiable)

The CPU **must not rely on fixed-cycle halts** (`halt_cycles`, `cycles_remaining`) to model DMA.

Constraints:

* IRQ recognition **must still occur at instruction boundaries**
* DMA must not suppress or delay IRQ entry
* If DMA stalls are present, they must behave like **RDY-based bus stalls**, not instruction-level halts

Any logic that blocks IRQ recognition due to DMA is incorrect.

---

## Forbidden Behaviors

Claude **must not** implement any of the following:

* Gating IRQ latching on the I flag
* Fetching an opcode before checking for interrupts in `FetchOpcode`
* Delaying IRQ entry until after an extra instruction executes
* Masking the test failure by altering DMC timing or IRQ clearing behavior
* Introducing heuristic delays instead of enforcing cycle-accurate behavior

---

## Acceptance Criteria

This requirement is satisfied **only if**:

* AccuracyCoins **Interrupt Flag Latency (code 1)** passes
* The CPU enters the IRQ sequence immediately at the correct boundary
* No opcode executes after a DMC IRQ becomes pending
* Interrupt entry begins with a dummy read at the current PC

---

## Scope of Allowed Changes

Claude may modify:

* `poll_interrupts`
* `FetchOpcode` handling
* IRQ latch semantics
* Transition logic into `InterruptSequence`

Claude may **not**:

* Modify AccuracyCoins tests
* Change DMC IRQ generation rules
* Remove cycle accuracy guarantees

---

