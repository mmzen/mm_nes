use std::collections::HashMap;
use std::{fmt, io};
use std::cell::RefCell;
use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::thread::sleep;
use std::time::Duration;
use lazy_static::lazy_static;
use log::{debug, error, info};
use crate::cpu::{CPU, CpuError};
use crate::memory::{Memory, MemoryError};

//const CLOCK_HZ: usize = 1_789_773;
const STACK_BASE_ADDRESS: u16 = 0x0100;
const STACK_END_ADDRESS: u16 = 0x01FF;


lazy_static! {
    static ref INSTRUCTIONS_TABLE: HashMap<u8, Instruction> = {
        let mut map = HashMap::<u8, Instruction>::new();

        macro_rules! add_instruction {
            ($map:ident, $opcode:expr, $op:ident, $addr_mode:ident, $bytes:expr, $cycles:expr, $exec:ident) => {
                $map.insert($opcode, Instruction {
                    opcode: OpCode::$op,
                    addressing_mode: AddressingMode::$addr_mode,
                    bytes: $bytes,
                    cycles: $cycles,
                    execute: Instruction::$exec,
                })
            };
        }

        include!("instructions_macro.rs");
        map
    };
}

#[derive(Debug)]
enum OpCode {
    ORA, AND, EOR, ADC, STA, LDA, CMP, SBC, ASL, ROL, LSR, ROR, STX, LDX, DEC, INC, JMP, STY, LDY,
    CPY, CPX, BIT, BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ, BRK, JSR, RTI, RTS, PHP, PLP, PHA, PLA,
    DEY, TAY, INY, INX, CLC, SEC, CLI, SEI, TYA, CLV, CLD, SED, TXA, TXS, TAX, TSX, DEX, NOP
}

#[derive(Debug)]
enum Operand {
    Byte(u8),
    Address(u16),
    AddressAndEffectiveAddress(u16, u16),
    Accumulator,
    None
}

impl Display for Operand {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Byte(val) => write!(f, "byte: 0x{:02X}", val),
            Operand::Address(addr) => write!(f, "word: 0x{:04X}", addr),
            Operand::Accumulator => write!(f, "accumulator"),
            Operand::None => { write!(f, "none") }
            Operand::AddressAndEffectiveAddress(addr, effective) => {
                write!(f, "address: 0x{:04X}, effective address: 0x{:04X}", addr, effective)
            }
        }
    }
}


#[derive(Debug)]
enum AddressingMode {
    Implicit,               // implicit addressing mode
    Accumulator,            // val = A
    Immediate,              // val = arg
    ZeroPage,               // val = PEEK(arg)
    ZeroPageIndexedX,       // val = PEEK((arg + X) % 256)
    ZeroPageIndexedY,       // val = PEEK((arg + Y) % 256)
    Absolute,               // val = PEEK(arg_16), LSb then MSb
    AbsoluteIndexedX,       // val = PEEK(arg + X)
    AbsoluteIndexedY,       // val = PEEK(arg + Y)
    Relative,               // val = arg_8 offset from pc
    Indirect,               // val = peek16(arg_16)
    IndirectIndexedX,       // val = PEEK(PEEK((arg + X) % 256) + PEEK((arg + X + 1) % 256) * 256)
    IndirectIndexedY,       // val = PEEK(PEEK(arg) + PEEK((arg + 1) % 256) * 256 + Y)
}

struct Instruction {
    opcode: OpCode,
    addressing_mode: AddressingMode,
    bytes: usize,
    cycles: usize,
    execute: fn(&Instruction, &mut Cpu6502, &Operand) -> Result<(), CpuError>,
}

impl Instruction {

    fn detect_overflow_add(lhs: u8, rhs: u8, sum: u8) -> bool {
        ((lhs & 0x80 == 0) && (rhs & 0x80 == 0) && (sum & 0x80 != 0)) ||
            ((lhs & 0x80 != 0) && (rhs & 0x80 != 0) && (sum & 0x80 == 0))
    }

    fn detect_overflow_sub(lhs: u8, rhs: u8, sum: u8) -> bool {
        ((lhs & 0x80 == 0) && (rhs & 0x80 != 0) && (sum & 0x80 != 0)) ||
            ((lhs & 0x80 != 0) && (rhs & 0x80 == 0) && (sum & 0x80 == 0))
    }

    fn detect_carry_add(lhs: u8, rhs: u8, carry_in: bool) -> bool {
        lhs as u16 + rhs as u16 + carry_in as u16 > 0xFF
    }

    fn detect_carry_sub(value0: u8, value1: u8, carry_in: bool) -> bool {
        value0 >= (value1.wrapping_add(!carry_in as u8))
    }

    fn get_operand_byte_value(&self, cpu: &Cpu6502, operand: &Operand) -> Result<u8, CpuError> {

        let result = match operand {
            Operand::Accumulator => {
                Ok(cpu.registers.a)
            },
            Operand::Byte(value) => {
                Ok(*value)
            },
            Operand::Address(addr) => {
                let value = cpu.memory.read_byte(*addr)?;
                Ok(value)
            },
            Operand::AddressAndEffectiveAddress(_, effective) => {
                let value = cpu.memory.read_byte(*effective)?;
                Ok(value)
            }
            Operand::None => {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        };

        result
    }

    fn get_operand_word_value(&self, _: &Cpu6502, operand: &Operand) -> Result<u16, CpuError> {

        let result = match operand {
            Operand::Address(addr) => {
                let value = *addr;
                Ok(value)
            },
            Operand::AddressAndEffectiveAddress(_, effective) => {
                let value = *effective;
                Ok(value)
            },
            _ => Err(CpuError::InvalidOperand(format!("{}", operand)))
        };

        result
    }

    fn adc_add_memory_to_accumulator_with_carry(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };

        let result = cpu.registers.a as u16 + value as u16 + carry_in as u16;

        cpu.registers.set_status(StatusFlag::Carry, result > 0xFF);
        cpu.registers.set_status(StatusFlag::Zero, result & 0xFF == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        let overflow = !(cpu.registers.a ^ value) & (cpu.registers.a ^ result as u8) & 0x80;
        cpu.registers.set_status(StatusFlag::Overflow, overflow != 0);

        cpu.registers.a = result as u8;

        Ok(())
    }

    fn and_and_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.a = cpu.registers.a & value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(())
    }

    fn asl_shift_left_one_bit(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {

        let (original_value, result) = match operand {
            Operand::Accumulator => {
                let original_value = cpu.registers.a;
                let result = original_value << 1;
                cpu.registers.a = result;
                (original_value, result)
            },

            Operand::Address(addr) |
            Operand::AddressAndEffectiveAddress(_, addr) =>{
                let original_value = cpu.memory.read_byte(*addr)?;
                let result = original_value << 1;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },

            _ => {
                return Err(CpuError::InvalidOperand(format!("{}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(())
    }

    fn bcc_branch_on_carry_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if !cpu.registers.get_status(StatusFlag::Carry) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn bcs_branch_on_carry_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if cpu.registers.get_status(StatusFlag::Carry) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn beq_branch_on_result_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if cpu.registers.get_status(StatusFlag::Zero) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn bit_test_bits_in_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if let Operand::Address(addr) = operand {
            let value = cpu.memory.read_byte(*addr)?;
            let result = cpu.registers.a & value;
            cpu.registers.set_status(StatusFlag::Zero, result == 0);
            cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);
            cpu.registers.set_status(StatusFlag::Overflow, value & 0x40 != 0);
            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{}", operand)))
        }
    }

    fn bmi_branch_on_result_minus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if cpu.registers.get_status(StatusFlag::Negative) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn bne_branch_on_result_not_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;

        if !cpu.registers.get_status(StatusFlag::Zero) {
            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;
            Ok(())
        } else {
            Ok(())
        }
    }

    fn bpl_branch_on_result_plus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if !cpu.registers.get_status(StatusFlag::Negative) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn brk_force_break(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.p |= StatusFlag::BreakCommand.bits();

        let next_pc = cpu.registers.safe_pc_add(2)?;

        cpu.push_stack((next_pc >> 8) as u8)?;
        cpu.push_stack((next_pc & 0xFF) as u8)?;
        cpu.push_stack(cpu.registers.p)?;

        cpu.registers.set_status(StatusFlag::InterruptDisable, true);
        cpu.registers.pc = cpu.memory.read_word(0xFFFE)?;

        Ok(())
    }

    fn bvc_branch_on_overflow_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if !cpu.registers.get_status(StatusFlag::Overflow) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn bvs_branch_on_overflow_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if cpu.registers.get_status(StatusFlag::Overflow) {
            if let Operand::Address(addr) = operand {
                cpu.registers.pc = *addr;
                Ok(())
            } else {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        } else {
            Ok(())
        }
    }

    fn clc_clear_carry_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::Carry, false);
        Ok(())
    }

    fn cld_clear_decimal_mode(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::DecimalMode, false);
        Ok(())
    }

    fn cli_clear_interrupt_disable_bit(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::InterruptDisable, false);
        Ok(())
    }

    fn clv_clear_overflow_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::Overflow, false);
        Ok(())
    }

    fn cmp_compare_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        let result = cpu.registers.a.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.a);

        Ok(())
    }

    fn cpx_compare_memory_and_index_x(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        let result = cpu.registers.x.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.x);

        Ok(())
    }

    fn cpy_compare_memory_and_index_y(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        let result = cpu.registers.y.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.y);

        Ok(())
    }

    fn dec_decrement_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if let Operand::Address(addr) = operand {
            let mut value = cpu.memory.read_byte(*addr)?;

            value = value.wrapping_sub(1);
            cpu.memory.write_byte(*addr, value)?;

            cpu.registers.set_status(StatusFlag::Zero, value == 0);
            cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("Expected a memory address, got: {:?}", operand)))
        }
    }

    fn dex_decrement_index_x_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.x = cpu.registers.x.wrapping_sub(1);
        cpu.registers.set_status(StatusFlag::Zero,  cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative,  cpu.registers.x & 0x80 != 0);
        Ok(())
    }

    fn dey_decrement_index_y_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.y = cpu.registers.y.wrapping_sub(1);
        let y = cpu.registers.y;
        cpu.registers.set_status(StatusFlag::Zero, y == 0);
        cpu.registers.set_status(StatusFlag::Negative, y & 0x80 != 0);
        Ok(())
    }

    fn eor_exclusive_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.a = cpu.registers.a ^ value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(())
    }

    fn inc_increment_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        if let Operand::Address(addr) = operand {
            let mut value = cpu.memory.read_byte(*addr)?;

            value = value.wrapping_add(1);
            cpu.memory.write_byte(*addr, value)?;

            cpu.registers.set_status(StatusFlag::Zero, value == 0);
            cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{}", operand)))
        }
    }

    fn inx_increment_index_x_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.x = cpu.registers.x.wrapping_add(1);
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);
        Ok(())
    }

    fn iny_increment_index_y_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.y = cpu.registers.y.wrapping_add(1);
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);
        Ok(())
    }

    fn jmp_jump_to_new_location(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;

        debug!("preparing to jump to absolute address {:04X}", addr);
        cpu.registers.pc = addr;
        Ok(())
    }

    fn jsr_jump_to_new_location_saving_return_address(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;
        let pc = cpu.registers.safe_pc_add(2)?;

        cpu.push_stack((pc >> 8) as u8)?;
        cpu.push_stack(pc as u8)?;

        cpu.registers.pc = addr;
        Ok(())
    }

    fn lda_load_accumulator_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(())
    }

    fn ldx_load_index_x_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.x = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(())
    }

    fn ldy_load_index_y_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.y = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(())
    }

    fn lsr_shift_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Operand::Accumulator => {
                let original_value = cpu.registers.a;
                let result = original_value >> 1;
                cpu.registers.a = result;
                (original_value, result)
            },

            Operand::Address(addr) |
            Operand::AddressAndEffectiveAddress(_, addr) =>{
                let original_value = cpu.memory.read_byte(*addr)?;
                let result = original_value >> 1;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },

            _ => {
                return Err(CpuError::InvalidOperand(format!("{}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, false);
        Ok(())
    }

    fn nop_no_operation(&self, _: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        Ok(())
    }

    fn ora_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;

        cpu.registers.a = cpu.registers.a | value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(())
    }

    fn pha_push_accumulator_on_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let value = cpu.registers.a;
        cpu.push_stack(value)?;
        Ok(())
    }

    fn php_push_processor_status_on_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let status = cpu.registers.p | 0x30;
        cpu.push_stack(status)?;
        Ok(())
    }

    fn pla_pull_accumulator_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let value = cpu.pop_stack()?;

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, value == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

        Ok(())
    }

    fn plp_pull_processor_status_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let status = cpu.pop_stack()?;
        cpu.registers.p = (status & 0xCF) | 0x20;
        Ok(())
    }

    fn rol_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Operand::Accumulator => {
                let original_value = cpu.registers.a;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };
                let result = (original_value << 1) | carry_in;
                cpu.registers.a = result;
                (original_value, result)
            },

            Operand::Address(addr) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };
                let result = (original_value << 1) | carry_in;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },
            _ => {
                return Err(CpuError::InvalidOperand(format!("{}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        Ok(())
    }

    fn ror_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Operand::Accumulator => {
                let original_value = cpu.registers.a;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
                let result = (original_value >> 1) | carry_in;
                cpu.registers.a = result;
                (original_value, result)
            },
            Operand::Address(addr) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
                let result = (original_value >> 1) | carry_in;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },
            _ => {
                return Err(CpuError::InvalidOperand(format!("{}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        Ok(())
    }

    fn rti_return_from_interrupt(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let status = cpu.pop_stack()?;
        cpu.registers.p = (status & 0xCF) | 0x20;

        let pcl = cpu.pop_stack()?;
        let pch = cpu.pop_stack()?;

        cpu.registers.pc = (pch as u16) << 8 | pcl as u16;

        Ok(())
    }

    fn rts_return_from_subroutine(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        let pcl = cpu.pop_stack()?;
        let pch = cpu.pop_stack()?;

        cpu.registers.pc = (pch as u16) << 8 | pcl as u16;
        cpu.registers.pc = cpu.registers.safe_pc_add(1)?;

        Ok(())
    }

    fn sbc_subtract_memory_from_accumulator_with_borrow(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let value = self.get_operand_byte_value(cpu, operand)?;
        let borrow = !cpu.registers.get_status(StatusFlag::Carry) as u8;

        let temp = value.wrapping_add(borrow);
        let result = cpu.registers.a.wrapping_sub(temp);

        cpu.registers.set_status(StatusFlag::Carry, cpu.registers.a >= temp);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80!= 0);

        let overflow = ((cpu.registers.a ^ result) & 0x80 != 0) && ((cpu.registers.a ^ value) & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Overflow, overflow);
        cpu.registers.a = result;

        Ok(())
    }

    fn sec_set_carry_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::Carry, true);
        Ok(())
    }

    fn sed_set_decimal_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::DecimalMode, true);
        Ok(())
    }

    fn sei_set_interrupt_disable_status(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.set_status(StatusFlag::InterruptDisable, true);
        Ok(())
    }

    fn sta_store_accumulator_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;
        cpu.memory.write_byte(addr, cpu.registers.a)?;

        Ok(())
    }

    fn stx_store_index_x_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;
        cpu.memory.write_byte(addr, cpu.registers.x)?;

        Ok(())
    }

    fn sty_store_index_y_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<(), CpuError> {
        let addr = self.get_operand_word_value(cpu, operand)?;
        cpu.memory.write_byte(addr, cpu.registers.y)?;

        Ok(())
    }

    fn tax_transfer_accumulator_to_index_x(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.x = cpu.registers.a;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);
        Ok(())
    }

    fn tay_transfer_accumulator_to_index_y(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.y = cpu.registers.a;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);
        Ok(())
    }

    fn tsx_transfer_stack_pointer_to_index_x(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.x = cpu.registers.sp;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);
        Ok(())
    }

    fn txa_transfer_index_x_to_accumulator(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.a = cpu.registers.x;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);
        Ok(())
    }

    fn txs_transfer_index_x_to_stack_register(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.sp = cpu.registers.x;
        Ok(())
    }

    fn tya_transfer_index_y_to_accumulator(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<(), CpuError> {
        cpu.registers.a = cpu.registers.y;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);
        Ok(())
    }
}

#[derive(Debug, Copy, Clone)]
enum StatusFlag {
    Carry = 0x01,
    Zero = 0x02,
    InterruptDisable = 0x04,
    DecimalMode = 0x08,
    BreakCommand = 0x10,
    Unused = 0x20,
    Overflow = 0x40,
    Negative = 0x80,
}

impl StatusFlag {
    fn bits(self) -> u8 {
        self as u8
    }
}

#[derive(Debug)]
struct Registers  {
    a: u8,      // Accumulator register
    x: u8,      // Index register X
    y: u8,      // Index register Y
    p: u8,      // Status register
    sp: u8,     // Stack pointer
    pc: u16,    // Program counter
}

impl Registers {

    fn set_status(&mut self, flag: StatusFlag, value: bool) {
        if value {
            debug!("setting status flag: {:?}, {:04X}", flag, flag.bits());
            self.p |= flag.bits();
        } else {
            debug!("clearing status flag: {:?}, {:04X}", flag, flag.bits());
            self.p &= !flag.bits();
        }
    }

    fn get_status(&self, flag: StatusFlag) -> bool {
        debug!("status flag: {:?}, {:04X}", flag, flag.bits());
        (self.p & flag.bits()) != 0
    }

    fn safe_pc_add(&self, n: i16) -> Result<u16, CpuError> {
        let pc = self.pc;
        let pc = pc.checked_add_signed(n)
            .ok_or(MemoryError::OutOfBounds(self.pc))?;

        Ok(pc)
    }
}

pub struct Cpu6502 {
    registers: Registers,
    memory: Box<dyn Memory>,
    instructions_executed: u64,
    tracer: Tracer,
}

impl CPU for Cpu6502 {
    fn reset(&mut self) -> Result<(), CpuError> {
        info!("resetting CPU");

        self.registers.a = 0;
        self.registers.x = 0;
        self.registers.y = 0;
        self.registers.p = 0;
        self.registers.set_status(StatusFlag::InterruptDisable, true);
        self.registers.set_status(StatusFlag::Unused, true);
        self.registers.sp = 0xFD;
        self.registers.pc = 0xFFFC;
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), CpuError> {
        info!("initializing CPU");

        self.reset()?;
        //self.memory.initialize()?;
        Ok(())
    }

    fn panic(&self, error: &CpuError) {
        error!("fatal exception: {}", error);
        self.dump_registers();
        self.dump_flags();
        //self.dump_memory();
        info!("number of instructions executed: {} ({:.1} %)", self.instructions_executed, (self.instructions_executed as f32 / 8991_f32) * 100_f32);
    }

    fn dump_registers(&self) {
        info!("CPU registers dump:");
        info!("- P (status): {:08b}", self.registers.p);
        info!("- A (accumulator): 0x{:02X}", self.registers.a);
        info!("- X (index): 0x{:02X}", self.registers.x);
        info!("- Y (index): 0x{:02X}", self.registers.y);
        info!("- SP (stack pointer): 0x{:02X}", self.registers.sp);
        info!("- PC (program counter): 0x{:04X}", self.registers.pc);
    }

    fn dump_flags(&self) {
        info!("CPU flags dump:");
        info!("- carry: {}", self.registers.p & StatusFlag::Carry.bits() != 0);
        info!("- zero: {}", self.registers.p & StatusFlag::Zero.bits() != 0);
        info!("- interrupt disable: {}", self.registers.p & StatusFlag::InterruptDisable.bits() != 0);
        info!("- decimal Mode: {}", self.registers.p & StatusFlag::DecimalMode.bits() != 0);
        info!("- break Command: {}", self.registers.p & StatusFlag::BreakCommand.bits() != 0);
        info!("- overflow: {}", self.registers.p & StatusFlag::Overflow.bits() != 0);
        info!("- negative: {}", self.registers.p & StatusFlag::Negative.bits() != 0);
    }

    fn dump_memory(&self) {
        self.memory.dump();
    }

    fn run(&mut self) -> Result<(), CpuError> {
        info!("running CPU ...");

        loop {
            debug!("pc: 0x{:04X}", self.registers.pc);
            let original_pc = self.registers.pc;

            let byte = self.memory.read_byte(self.registers.pc)?;
            let instruction = Cpu6502::decode_instruction(byte)?;
            let operand = self.fetch_operand(instruction)?;

            self.tracer.trace(&self, &instruction, &operand)?;
            self.execute_instruction(instruction, &operand)?;

            if original_pc == self.registers.pc {
                self.registers.pc = self.registers.safe_pc_add(instruction.bytes as i16)?;
            }

            self.instructions_executed += 1;
            //sleep(Duration::from_millis(1));
        }
    }

    fn run_start_at(&mut self, address: u16) -> Result<(), CpuError> {
        self.registers.pc = address;

        debug!("pc set to address 0x{:04X} ...", address);
        self.run()
    }
}

impl Cpu6502 {
    pub fn new(memory: Box<dyn Memory>, trace_file: Option<File>) -> Self {
        Cpu6502 {
            registers: Registers {
                a: 0,
                x: 0,
                y: 0,
                p: 0,
                sp: 0,
                pc: 0
            },
            memory,
            instructions_executed: 0,
            tracer: Tracer::new_with_file(trace_file)
        }
    }

    fn is_valid_stack_addr(&self, addr: u16) -> Result<(), CpuError> {
        if addr > STACK_END_ADDRESS {
            Err(CpuError::StackOverflow(addr))
        } else if addr < STACK_BASE_ADDRESS {
            Err(CpuError::StackUnderflow(addr))
        } else {
            Ok(())
        }
    }

    fn push_stack(&mut self, value: u8) -> Result<(), CpuError> {
        let mut addr = STACK_BASE_ADDRESS + self.registers.sp as u16;

        debug!("sp (before push): 0x{:02X}, pushing at 0x{:02X}, value {:02X}", self.registers.sp, addr, value);
        self.memory.write_byte(addr, value)?;

        addr = addr - 1;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        debug!("sp (after push): 0x{:02X}", self.registers.sp);

        Ok(())
    }

    fn pop_stack(&mut self) -> Result<u8, CpuError> {
        let addr = STACK_BASE_ADDRESS + self.registers.sp as u16 + 1;

        debug!("sp (before pop): 0x{:02X}, popping at 0x{:02X}", self.registers.sp, addr);
        let value = self.memory.read_byte(addr)?;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        debug!("sp (after pop): 0x{:02X}, popped value {:02X}", self.registers.sp, value);

        Ok(value)
    }

    fn decode_instruction<'a>(byte: u8) -> Result<&'a Instruction, CpuError> {
        let aaa = (byte & 0b1110_0000) >> 2;
        let cc = byte & 0b0000_0011;
        let opcode = aaa | cc;

        debug!("decoded instruction: 0x{:02X}: opcode: 0x{:02X}", byte, opcode);

        if let Some(instruction) = INSTRUCTIONS_TABLE.get(&byte) {
            Ok(instruction)
        } else {
            Err(CpuError::IllegalInstruction(byte))
        }
    }

    fn fetch_operand(&self, instruction: &Instruction) -> Result<Operand, CpuError> {

        debug!("fetching operand for instruction: {:?}, {:?}", instruction.opcode, instruction.addressing_mode);

        let operand = match instruction.addressing_mode {
            AddressingMode::Implicit => {
                Operand::None
            },

            AddressingMode::Accumulator => {
                Operand::Accumulator
            },

            AddressingMode::Immediate => {
                let pc = self.registers.safe_pc_add(1)?;

                Operand::Byte(self.memory.read_byte(pc)?)
            },

            AddressingMode::Absolute => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;

                Operand::Address(addr)
            },

            AddressingMode::AbsoluteIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.x as u16);

                Operand::AddressAndEffectiveAddress(addr, effective_addr)
            }

            AddressingMode::AbsoluteIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.y as u16);

                Operand::AddressAndEffectiveAddress(addr, effective_addr)
            }

            AddressingMode::ZeroPage => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;

                Operand::Address(addr as u16)
            },

            AddressingMode::ZeroPageIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.x) as u16;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr)
            },

            AddressingMode::ZeroPageIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.y) as u16;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr)
            },

            AddressingMode::Indirect => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                let effective_addr = self.memory.read_word_with_page_wrap(addr)?;

                Operand::AddressAndEffectiveAddress(addr, effective_addr)
            },

            AddressingMode::IndirectIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indirect_addr = addr.wrapping_add(self.registers.x);
                let effective_addr = self.memory.read_word_with_page_wrap(indirect_addr as u16)?;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr)
            },

            AddressingMode::IndirectIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indirect_addr = self.memory.read_word_with_page_wrap(addr as u16)?;
                let effective_addr = indirect_addr.wrapping_add(self.registers.y as u16);

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr)
            },

            AddressingMode::Relative => {
                let pc = self.registers.safe_pc_add(1)?;
                let offset = self.memory.read_byte(pc)? as i16;
                let addr = self.registers.safe_pc_add(offset+2)?;

                Operand::Address(addr)
            },
        };

        debug!("fetched operand: {}", operand);
        Ok(operand)
    }

    fn execute_instruction(&mut self, instruction: &Instruction, operand: &Operand) -> Result<(), CpuError> {
        
        debug!("executing instruction: opcode: {:?}, addressing mode: {:?}, operand: {}",
            instruction.opcode, instruction.addressing_mode, operand);

        (instruction.execute)(instruction, self, operand)
    }
}

pub struct Tracer {
    trace: RefCell<Box<dyn Write>>,
}

impl Tracer {
    fn new_with_file(trace_file: Option<File>) -> Tracer {
        let writer: Box<dyn Write> = if let Some(trace_file) = trace_file {
            Box::new(trace_file)
        } else {
            Box::new(io::stdout())
        };

        Tracer { trace: RefCell::new(writer) }
    }

    fn trace(&self, cpu: &Cpu6502, instruction: &Instruction, operand: &Operand) -> Result<(), CpuError> {
        let a = format!("{:04X}", cpu.registers.pc);

        let mut b = format!("{:02X}", cpu.memory.read_byte(cpu.registers.pc)?);
        for i in 1..instruction.bytes {
            let o = format!(" {:02X}", cpu.memory.read_byte(cpu.registers.pc + i as u16)?);
            b.push_str(&o);
        }

        let c0 = format!("{:?}", &instruction.opcode);

        let c1 = match (&instruction.addressing_mode, operand, &instruction.opcode) {
            (AddressingMode::Implicit, _, _) => { "".to_string() },

            (AddressingMode::Accumulator, _, _) =>
                "A".to_string(),

            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JMP) |
            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JSR) =>
                format!("${:04X}", *addr),

            (AddressingMode::Absolute, Operand::Address(addr), _) =>
                format!("${:04X} = {:02X}", *addr, cpu.memory.read_byte(*addr)?),

            (AddressingMode::Relative, Operand::Address(addr), _) =>
                format!("${:04X}", *addr),

            (AddressingMode::ZeroPage, Operand::Address(addr), _) =>
                format!("${:02X} = {:02X}", *addr as u8, cpu.memory.read_byte(*addr)?),

            (AddressingMode::AbsoluteIndexedX, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("${:04X},X @ {:04X} = {:02X}", *addr, *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::AbsoluteIndexedY, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("${:04X},Y @ {:04X} = {:02X}", *addr, *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedX, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("${:02X},X @ {:02X} = {:02X}", *addr, *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedY, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("${:02X},Y @ {:02X} = {:02X}", *addr, *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::Indirect, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("(${:04X}) = {:04X}", *addr, effective),

            (AddressingMode::IndirectIndexedX, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("(${:02X},X) @ {:02X} = {:04X} = {:02X}", *addr, (*addr as u8).wrapping_add(cpu.registers.x), *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::IndirectIndexedY, Operand::AddressAndEffectiveAddress(addr, effective), _) =>
                format!("(${:02X}),Y = {:04X} @ {:04X} = {:02X}", *addr, effective.wrapping_sub(cpu.registers.y as u16), *effective, cpu.memory.read_byte(*effective)?),

            (AddressingMode::Immediate, Operand::Byte(byte), _) =>
                format!("#${:02X}", byte),

            _ => {
                return Err(CpuError::InvalidOperand(
                    format!("could not format instruction and operand: {:?}, {:?}",
                            &instruction.addressing_mode, operand)
                ))
            }
        };
        let c = format!("{} {}", c0, c1);

        let d = format!("A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
                        cpu.registers.a, cpu.registers.x, cpu.registers.y, cpu.registers.p, cpu.registers.sp);

        let mut output = self.trace.borrow_mut();

        write!(output, "{:<6}{:<10}", a, b)?;
        write!(output, "{:<padding$}", c, padding = 32)?;
        write!(output, "{}", d)?;
        writeln!(output)?;

        Ok(())
    }
}