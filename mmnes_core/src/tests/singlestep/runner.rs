// Authorship: Human 0% | Claude 100%
//! Test runner for SingleStepTests validation
//!
//! Executes test cases against the CPU and validates both final state
//! and cycle-accurate bus activity.

use std::cell::RefCell;
use std::rc::Rc;
use crate::cpu::CPU;
use crate::cpu_6502::Cpu6502;
use super::{TestCase, TestResult, StateError, CycleError, BusCycle};
use super::tracing_bus::TracingBus;

/// Run a single test case and return the result
pub fn run_test_case(test: &TestCase) -> TestResult {
    let mut result = TestResult::new(test.name.clone());

    // 1. Create TracingBus and initialize RAM from test.initial.ram
    let bus = Rc::new(RefCell::new(TracingBus::new()));
    bus.borrow_mut().load_memory(&test.initial.ram);

    // 2. Create CPU with TracingBus
    let mut cpu = Cpu6502::new(bus.clone());

    // 3. Set CPU state from test.initial (pc, s, a, x, y, p)
    cpu.set_state_for_test(
        test.initial.pc,
        test.initial.s,
        test.initial.a,
        test.initial.x,
        test.initial.y,
        test.initial.p,
    );

    // 4. Clear trace, execute step_instruction()
    bus.borrow().clear_trace();

    let step_result = cpu.step_instruction();
    if let Err(_e) = step_result {
        result.add_state_error(StateError::Register {
            name: "execution",
            expected: 0,
            actual: 0,
        });
        // Add a note about the error in cycle errors
        result.add_cycle_error(CycleError::CountMismatch {
            expected: test.cycles.len(),
            actual: 0,
        });
        return result;
    }

    // 5. Compare final CPU state with test.final_state
    validate_cpu_state(&cpu, &test.final_state, &mut result);

    // 6. Validate final memory state
    validate_memory_state(&bus.borrow(), &test.final_state.ram, &mut result);

    // 7. Compare bus trace with test.cycles
    let trace = bus.borrow().get_trace();
    validate_cycles(&trace, &test.cycles, &mut result);

    result
}

/// Validate CPU register state against expected final state
fn validate_cpu_state(cpu: &Cpu6502, expected: &super::CpuState, result: &mut TestResult) {
    if cpu.get_pc() != expected.pc {
        result.add_state_error(StateError::Register {
            name: "PC",
            expected: expected.pc,
            actual: cpu.get_pc(),
        });
    }

    if cpu.get_sp() != expected.s {
        result.add_state_error(StateError::Register {
            name: "S",
            expected: expected.s as u16,
            actual: cpu.get_sp() as u16,
        });
    }

    if cpu.get_a() != expected.a {
        result.add_state_error(StateError::Register {
            name: "A",
            expected: expected.a as u16,
            actual: cpu.get_a() as u16,
        });
    }

    if cpu.get_x() != expected.x {
        result.add_state_error(StateError::Register {
            name: "X",
            expected: expected.x as u16,
            actual: cpu.get_x() as u16,
        });
    }

    if cpu.get_y() != expected.y {
        result.add_state_error(StateError::Register {
            name: "Y",
            expected: expected.y as u16,
            actual: cpu.get_y() as u16,
        });
    }

    if cpu.get_status() != expected.p {
        result.add_state_error(StateError::Register {
            name: "P",
            expected: expected.p as u16,
            actual: cpu.get_status() as u16,
        });
    }
}

/// Validate memory state against expected final RAM values
fn validate_memory_state(bus: &TracingBus, expected_ram: &[(u16, u8)], result: &mut TestResult) {
    for &(addr, expected_value) in expected_ram {
        let actual_value = bus.peek(addr);
        if actual_value != expected_value {
            result.add_state_error(StateError::Memory {
                address: addr,
                expected: expected_value,
                actual: actual_value,
            });
        }
    }
}

/// Validate bus cycle trace against expected cycles
fn validate_cycles(actual: &[BusCycle], expected: &[BusCycle], result: &mut TestResult) {
    // First check count
    if actual.len() != expected.len() {
        result.add_cycle_error(CycleError::CountMismatch {
            expected: expected.len(),
            actual: actual.len(),
        });
    }

    // Compare each cycle
    let max_len = actual.len().max(expected.len());
    for i in 0..max_len {
        match (actual.get(i), expected.get(i)) {
            (Some(a), Some(e)) => {
                if a != e {
                    result.add_cycle_error(CycleError::CycleMismatch {
                        cycle_index: i,
                        expected: e.clone(),
                        actual: a.clone(),
                    });
                }
            }
            (None, Some(e)) => {
                result.add_cycle_error(CycleError::MissingCycle {
                    cycle_index: i,
                    expected: e.clone(),
                });
            }
            (Some(a), None) => {
                result.add_cycle_error(CycleError::ExtraCycle {
                    cycle_index: i,
                    actual: a.clone(),
                });
            }
            (None, None) => unreachable!(),
        }
    }
}

/// Run all test cases from a JSON file and return summary statistics
pub fn run_opcode_tests(json_content: &str) -> (usize, usize, Vec<TestResult>) {
    let tests: Vec<TestCase> = serde_json::from_str(json_content)
        .expect("Failed to parse test JSON");

    let mut passed = 0;
    let mut failed = 0;
    let mut failed_results = Vec::new();

    for test in &tests {
        let result = run_test_case(test);
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
            failed_results.push(result);
        }
    }

    (passed, failed, failed_results)
}

/// Run all test cases and return only the first N failures for debugging
pub fn run_opcode_tests_with_limit(json_content: &str, max_failures: usize) -> (usize, usize, Vec<TestResult>) {
    let tests: Vec<TestCase> = serde_json::from_str(json_content)
        .expect("Failed to parse test JSON");

    let mut passed = 0;
    let mut failed = 0;
    let mut failed_results = Vec::new();

    for test in &tests {
        let result = run_test_case(test);
        if result.passed {
            passed += 1;
        } else {
            failed += 1;
            if failed_results.len() < max_failures {
                failed_results.push(result);
            }
        }
    }

    (passed, failed, failed_results)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tests::singlestep::BusOperation;

    #[test]
    fn run_test_case_validates_simple_nop() {
        // NOP instruction: opcode EA, 2 cycles, no state change except PC
        // Implied mode does dummy read of next byte
        let test = TestCase {
            name: "ea NOP".to_string(),
            initial: super::super::CpuState {
                pc: 0x0200,
                s: 0xFD,
                a: 0x00,
                x: 0x00,
                y: 0x00,
                p: 0x24,
                ram: vec![(0x0200, 0xEA), (0x0201, 0x00)], // NOP opcode + dummy byte
            },
            final_state: super::super::CpuState {
                pc: 0x0201,
                s: 0xFD,
                a: 0x00,
                x: 0x00,
                y: 0x00,
                p: 0x24,
                ram: vec![(0x0200, 0xEA), (0x0201, 0x00)],
            },
            // 2 cycles: opcode fetch + dummy read of next byte
            cycles: vec![
                BusCycle { address: 0x0200, value: 0xEA, operation: BusOperation::Read },
                BusCycle { address: 0x0201, value: 0x00, operation: BusOperation::Read },
            ],
        };

        let result = run_test_case(&test);

        if !result.passed {
            println!("{}", result);
        }
        assert!(result.passed, "NOP test should pass");
    }

    #[test]
    fn run_test_case_detects_pc_mismatch() {
        let test = TestCase {
            name: "ea NOP bad pc".to_string(),
            initial: super::super::CpuState {
                pc: 0x0200,
                s: 0xFD,
                a: 0x00,
                x: 0x00,
                y: 0x00,
                p: 0x24,
                ram: vec![(0x0200, 0xEA), (0x0201, 0x00)],
            },
            final_state: super::super::CpuState {
                pc: 0x0999, // Wrong expected PC
                s: 0xFD,
                a: 0x00,
                x: 0x00,
                y: 0x00,
                p: 0x24,
                ram: vec![(0x0200, 0xEA), (0x0201, 0x00)],
            },
            // 2 cycles: opcode fetch + dummy read
            cycles: vec![
                BusCycle { address: 0x0200, value: 0xEA, operation: BusOperation::Read },
                BusCycle { address: 0x0201, value: 0x00, operation: BusOperation::Read },
            ],
        };

        let result = run_test_case(&test);
        assert!(!result.passed, "Should detect PC mismatch");
        assert!(!result.state_errors.is_empty());
    }
}
