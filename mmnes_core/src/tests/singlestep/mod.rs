// Authorship: Human 0% | Claude 100%
//! SingleStepTests integration for cycle-accurate 6502 CPU validation
//!
//! This module integrates the SingleStepTests/65x02 test suite to validate
//! the CPU implementation with full cycle-by-cycle bus activity verification.
//!
//! Test format from SingleStepTests:
//! ```json
//! {
//!   "name": "opcode operand1 operand2",
//!   "initial": { "pc": u16, "s": u8, "a": u8, "x": u8, "y": u8, "p": u8, "ram": [[addr, value], ...] },
//!   "final": { "pc": u16, "s": u8, "a": u8, "x": u8, "y": u8, "p": u8, "ram": [[addr, value], ...] },
//!   "cycles": [[address, value, "read"|"write"], ...]
//! }
//! ```

pub mod tracing_bus;
mod runner;
#[cfg(test)]
mod tests;

use serde::Deserialize;

/// Represents the CPU state (registers) at a point in time
#[derive(Debug, Clone, Deserialize)]
pub struct CpuState {
    pub pc: u16,
    pub s: u8,
    pub a: u8,
    pub x: u8,
    pub y: u8,
    pub p: u8,
    pub ram: Vec<(u16, u8)>,
}

/// Represents a single bus cycle operation
#[derive(Debug, Clone, PartialEq)]
pub struct BusCycle {
    pub address: u16,
    pub value: u8,
    pub operation: BusOperation,
}

/// Bus operation type
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum BusOperation {
    Read,
    Write,
}

/// A single test case from the SingleStepTests suite
#[derive(Debug, Clone, Deserialize)]
pub struct TestCase {
    pub name: String,
    pub initial: CpuState,
    #[serde(rename = "final")]
    pub final_state: CpuState,
    #[serde(deserialize_with = "deserialize_cycles")]
    pub cycles: Vec<BusCycle>,
}

/// Custom deserializer for cycles array: [[addr, value, "read"|"write"], ...]
fn deserialize_cycles<'de, D>(deserializer: D) -> Result<Vec<BusCycle>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de::Error;

    let raw: Vec<(u16, u8, String)> = Vec::deserialize(deserializer)?;
    raw.into_iter()
        .map(|(address, value, op)| {
            let operation = match op.as_str() {
                "read" => BusOperation::Read,
                "write" => BusOperation::Write,
                other => return Err(D::Error::custom(format!("unknown bus operation: {}", other))),
            };
            Ok(BusCycle { address, value, operation })
        })
        .collect()
}

/// Result of running a single test case
#[derive(Debug)]
pub struct TestResult {
    pub test_name: String,
    pub passed: bool,
    pub state_errors: Vec<StateError>,
    pub cycle_errors: Vec<CycleError>,
}

/// Error in final CPU/memory state
#[derive(Debug)]
pub enum StateError {
    Register { name: &'static str, expected: u16, actual: u16 },
    Memory { address: u16, expected: u8, actual: u8 },
}

/// Error in bus cycle activity
#[derive(Debug)]
pub enum CycleError {
    CountMismatch { expected: usize, actual: usize },
    CycleMismatch {
        cycle_index: usize,
        expected: BusCycle,
        actual: BusCycle
    },
    MissingCycle { cycle_index: usize, expected: BusCycle },
    ExtraCycle { cycle_index: usize, actual: BusCycle },
}

impl TestResult {
    pub fn new(test_name: String) -> Self {
        TestResult {
            test_name,
            passed: true,
            state_errors: Vec::new(),
            cycle_errors: Vec::new(),
        }
    }

    pub fn add_state_error(&mut self, error: StateError) {
        self.passed = false;
        self.state_errors.push(error);
    }

    pub fn add_cycle_error(&mut self, error: CycleError) {
        self.passed = false;
        self.cycle_errors.push(error);
    }
}

impl std::fmt::Display for TestResult {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.passed {
            write!(f, "PASS: {}", self.test_name)
        } else {
            writeln!(f, "FAIL: {}", self.test_name)?;
            for err in &self.state_errors {
                match err {
                    StateError::Register { name, expected, actual } => {
                        writeln!(f, "  Register {}: expected 0x{:04X}, got 0x{:04X}", name, expected, actual)?;
                    }
                    StateError::Memory { address, expected, actual } => {
                        writeln!(f, "  Memory[0x{:04X}]: expected 0x{:02X}, got 0x{:02X}", address, expected, actual)?;
                    }
                }
            }
            for err in &self.cycle_errors {
                match err {
                    CycleError::CountMismatch { expected, actual } => {
                        writeln!(f, "  Cycle count: expected {}, got {}", expected, actual)?;
                    }
                    CycleError::CycleMismatch { cycle_index, expected, actual } => {
                        writeln!(f, "  Cycle {}: expected {:?}, got {:?}", cycle_index, expected, actual)?;
                    }
                    CycleError::MissingCycle { cycle_index, expected } => {
                        writeln!(f, "  Cycle {}: missing, expected {:?}", cycle_index, expected)?;
                    }
                    CycleError::ExtraCycle { cycle_index, actual } => {
                        writeln!(f, "  Cycle {}: extra, got {:?}", cycle_index, actual)?;
                    }
                }
            }
            Ok(())
        }
    }
}
