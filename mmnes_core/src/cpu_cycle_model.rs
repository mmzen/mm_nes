// Authorship: Human 0% | Claude 100%
//! CPU Cycle-Accurate Bus Model
//!
//! This module defines the per-cycle bus activity for each 6502 instruction.
//! Each instruction is broken down into micro-operations that each take exactly
//! one CPU cycle and perform exactly one bus operation.
//!
//! Reference: http://www.oxyron.de/html/opcodes02.html
//!            http://nesdev.org/6502_cpu.txt

use crate::memory::MemoryError;

/// A micro-operation that takes exactly one CPU cycle.
/// Each micro-op performs exactly one bus read or write.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MicroOp {
    /// Fetch opcode at PC
    FetchOpcode,
    /// Fetch operand byte at PC+1 (immediate, zero page, etc.)
    FetchOperandByte,
    /// Fetch low byte of address at PC+1
    FetchAddressLow,
    /// Fetch high byte of address at PC+2
    FetchAddressHigh,
    /// Dummy read at PC+1 (for implied/accumulator modes)
    DummyReadNextByte,
    /// Read from zero page address
    ReadZeroPage { addr: u8 },
    /// Dummy read from zero page (for indexed zero page)
    DummyReadZeroPage { addr: u8 },
    /// Read from absolute address
    ReadAbsolute { addr: u16 },
    /// Dummy read from absolute address (for page crossing or RMW)
    DummyReadAbsolute { addr: u16 },
    /// Write to zero page address
    WriteZeroPage { addr: u8, value: u8 },
    /// Write to absolute address
    WriteAbsolute { addr: u16, value: u8 },
    /// Read from stack (for PLA, PLP, RTI, RTS)
    ReadStack { offset: u8 },
    /// Write to stack (for PHA, PHP, JSR, BRK)
    WriteStack { offset: u8, value: u8 },
    /// Dummy stack read (for stack operations)
    DummyStackRead,
    /// Read indirect pointer low byte
    ReadIndirectLow { ptr_addr: u8 },
    /// Read indirect pointer high byte
    ReadIndirectHigh { ptr_addr: u8 },
    /// Execute ALU operation (internal, no bus activity - but 6502 always reads)
    /// This performs a dummy read at some address
    Execute { dummy_addr: u16 },
}

/// Result of executing a micro-operation
#[derive(Debug, Clone, Default)]
pub struct MicroOpResult {
    /// Address accessed on bus
    pub address: u16,
    /// Data read/written
    pub data: u8,
    /// True if this was a read operation
    pub is_read: bool,
    /// True if this was a write operation
    pub is_write: bool,
    /// True if instruction is complete after this cycle
    pub instruction_complete: bool,
}

/// Cycle timing for each addressing mode.
/// Returns the sequence of micro-operations for a given addressing mode and instruction type.
pub fn get_micro_ops_for_instruction(
    opcode: u8,
    is_read: bool,      // LDA, LDX, etc.
    is_write: bool,     // STA, STX, etc.
    is_rmw: bool,       // INC, DEC, ASL, etc.
    is_branch: bool,    // BCC, BCS, etc.
    is_stack: bool,     // PHA, PLA, etc.
    is_jmp: bool,       // JMP
    is_jsr: bool,       // JSR
    is_rts: bool,       // RTS
    is_rti: bool,       // RTI
    is_brk: bool,       // BRK
) -> Vec<MicroOpTemplate> {
    // This function returns templates that will be filled in during execution
    // For now, return empty - will be populated as we implement each addressing mode
    vec![]
}

/// Template for a micro-operation sequence.
/// Some values are known at decode time, others are computed during execution.
#[derive(Debug, Clone)]
pub enum MicroOpTemplate {
    /// Fixed micro-op (all values known)
    Fixed(MicroOp),
    /// Need to compute address from operand
    ComputeAddress,
    /// Need to read from computed effective address
    ReadEffective,
    /// Need to write to computed effective address
    WriteEffective,
    /// Dummy read at computed effective address
    DummyEffective,
}

// =============================================================================
// Cycle counts for each addressing mode (for reference)
// =============================================================================
//
// Immediate (2 cycles):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch immediate value
//
// Zero Page (3 cycles for read, 3 for write):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch zero page address
//   Cycle 3: Read/Write from zero page
//
// Zero Page,X (4 cycles):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch zero page address
//   Cycle 3: Dummy read from ZP address (while adding X)
//   Cycle 4: Read/Write from (ZP + X) & 0xFF
//
// Absolute (4 cycles for read, 4 for write):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch low byte of address
//   Cycle 3: Fetch high byte of address
//   Cycle 4: Read/Write from absolute address
//
// Absolute,X (4-5 cycles for read, 5 for write):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch low byte of address
//   Cycle 3: Fetch high byte of address
//   Cycle 4: Read from (addr + X) & 0xFFFF (may be wrong page)
//   Cycle 5: Read from correct address (only if page crossed) / Always for write
//
// (Indirect,X) (6 cycles):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch pointer address
//   Cycle 3: Dummy read from pointer (while adding X)
//   Cycle 4: Read low byte of effective address from (ptr + X) & 0xFF
//   Cycle 5: Read high byte of effective address from (ptr + X + 1) & 0xFF
//   Cycle 6: Read/Write from effective address
//
// (Indirect),Y (5-6 cycles for read, 6 for write):
//   Cycle 1: Fetch opcode
//   Cycle 2: Fetch pointer address
//   Cycle 3: Read low byte of effective address from ptr
//   Cycle 4: Read high byte of effective address from (ptr + 1) & 0xFF
//   Cycle 5: Read from (effective + Y) (may be wrong page)
//   Cycle 6: Read from correct address (only if page crossed) / Always for write
//
// Read-Modify-Write (6 cycles for ZP, 7 for Absolute):
//   Additional cycles for the dummy write and final write
//
// =============================================================================
