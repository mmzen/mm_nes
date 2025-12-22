INTERRUPT FLAG LATENCY
---

# Requirement: Correct DMC IRQ Interrupt Latency (AccuracyCoins Code 1)

## Objective

Fix the **Interrupt Flag Latency** failure reported by AccuracyCoins (error code 1):

> *“An IRQ should occur when a DMC sample ends, the DMC IRQ is enabled, and the CPU's I flag is clear.”*

The emulator **must enter the IRQ interrupt sequence at the correct instruction boundary**, without executing an extra opcode.

---

## Required Behavioral Guarantees

### 1. IRQ Recognition Timing (Critical)

When a **DMC IRQ becomes asserted** and the CPU **I flag is clear**, the CPU **must begin the IRQ interrupt sequence at the next instruction boundary**, **before executing the next opcode**.

Specifically:

* The CPU **must not fetch and execute a new opcode** once an IRQ is recognized at an instruction boundary.
* Instead, the CPU **must immediately transition to the hardware interrupt sequence**.
* The first cycle of the interrupt sequence **must perform a dummy read from the current PC**, exactly as real 6502 hardware does.

---

### 2. FetchOpcode State Rules (Mandatory Change)

In the CPU state machine:

* During the `FetchOpcode` state:

    1. Interrupt lines **must be polled**
    2. If an interrupt is pending:

        * The CPU **must not fetch or decode the next opcode**
        * The CPU **must transition directly to `InterruptSequence { cycle = 1 }`**
        * The cycle must perform a **dummy read at the current PC**
    3. Only if **no interrupt is pending** may the CPU fetch and execute the next opcode

This behavior is mandatory for both IRQ and NMI (with NMI taking priority).

---

### 3. IRQ Latching Semantics

IRQ sampling **must not be gated by the I flag at poll time**.

Required behavior:

* The CPU must latch the **IRQ line level** independently of the I flag
* The I flag must be checked **only when deciding whether to enter the IRQ sequence**
* This ensures correct latency when:

    * IRQ is asserted while I=1
    * I is cleared shortly before the instruction boundary

This matches real 6502 behavior and avoids lost IRQs.

---

### 4. DMC IRQ Behavior (APU Side)

The DMC channel must:

* Assert its IRQ **when the sample ends and IRQ is enabled**
* Keep the IRQ asserted **until explicitly cleared**
* Clearing occurs only when:

    * `$4015` is written
    * `$4010` disables DMC IRQ

No edge-triggered or pulse-style IRQ behavior is allowed.

---

### 5. DMA Stall Interaction

The CPU **must not use a cycle countdown–based halt mechanism** for DMA (`halt_cycles`, `cycles_remaining`, etc.).

DMA stalling must:

* Be modeled as **RDY-style bus blocking**
* Allow interrupts to be recognized at instruction boundaries
* Never suppress IRQ entry timing

Any legacy halt logic that conflicts with this must be removed or disabled.

---

## Forbidden Behaviors

The following behaviors are explicitly disallowed:

* Executing one additional opcode after a DMC IRQ becomes pending
* Delaying IRQ handling until after an instruction completes when the IRQ was already pending at the boundary
* Gating IRQ latching on the I flag
* Using fixed-cycle halts for DMA instead of bus-level stalling

---

## Validation Criteria

This requirement is satisfied only if:

* AccuracyCoins **Interrupt Flag Latency** test passes (code 1 cleared)
* The IRQ sequence begins immediately at instruction boundaries
* A dummy opcode fetch occurs as part of interrupt entry
* No extra instruction executes before IRQ handling

---

## Implementation Scope

Claude is allowed to modify:

* CPU interrupt polling logic
* `FetchOpcode` state handling
* IRQ latch semantics
* Removal or isolation of legacy DMA halt code

Claude **must not**:

* Mask failures by delaying or suppressing IRQs
* Approximate timing instead of enforcing cycle-accurate behavior


