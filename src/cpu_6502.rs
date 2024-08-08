use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;
use lazy_static::lazy_static;
use log::{debug, error, info};
use crate::cpu::{CPU, CpuError};
use crate::memory::{Memory, MemoryError};

const CLOCK_HZ: usize = 1_789_773;

const STACK_BASE_ADDRESS: u16 = 0x0100;
const STACK_END_ADDRESS: u16 = 0x01FF;

#[derive(Debug)]
enum Value {
    Byte(u8),
    Word(u16),
    Accumulator
}

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



#[derive(Debug, Copy, Clone)]
enum StatusFlag {
    Carry = 0x01,
    Zero = 0x02,
    InterruptDisable = 0x04,
    DecimalMode = 0x08,
    BreakCommand = 0x10,
    Overflow = 0x40,
    Negative = 0x80,
}

impl StatusFlag {
    fn bits(self) -> u8 {
        self as u8
    }
}

#[derive(Debug)]
pub struct Registers  {
    pub a: u8,      // Accumulator register
    pub x: u8,      // Index register X
    pub y: u8,      // Index register Y
    pub p: u8,      // Status register
    pub sp: u8,     // Stack pointer
    pub pc: u16,    // Program counter
}

impl Registers {
    fn set_eval_status<F>(&mut self, flag: StatusFlag, predicate: F)
    where F: FnOnce() -> bool,
    {
        if predicate() {
            self.p |= flag.bits();
        } else {
            self.p &= !flag.bits();
        }
    }

    fn set_status(&mut self, flag: StatusFlag, value: bool) {
        if value {
            self.p |= flag.bits();
        } else {
            self.p &= !flag.bits();
        }
    }

    fn get_status(&self, flag: StatusFlag) -> bool {
        (self.p & flag.bits()) != 0
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

#[derive(Debug)]
enum OpCode {
    ORA,
    AND,
    EOR,
    ADC,
    STA,
    LDA,
    CMP,
    SBC,
    ASL,
    ROL,
    LSR,
    ROR,
    STX,
    LDX,
    DEC,
    INC,
    JMP,
    JMPA,
    STY,
    LDY,
    CPY,
    CPX,
    BIT,
    BPL,
    BMI,
    BVC,
    BVS,
    BCC,
    BCS,
    BNE,
    BEQ,
    BRK,
    JSRA,
    JSR,
    RTI,
    RTS,
    PHP,
    PLP,
    PHA,
    PLA,
    DEY,
    TAY,
    INY,
    INX,
    CLC,
    SEC,
    CLI,
    SEI,
    TYA,
    CLV,
    CLD,
    SED,
    TXA,
    TXS,
    TAX,
    TSX,
    DEX,
    NOP
}

struct Instruction {
    opcode: OpCode,
    addressing_mode: AddressingMode,
    bytes: usize,
    cycles: usize,
    execute: fn(&Instruction, &mut Cpu6502, &Option<Value>) -> Result<(), CpuError>,
}

struct InstructionTable<'a> {
    table: HashMap<(&'a OpCode, &'a AddressingMode), Instruction>
}

#[derive(Debug)]
pub struct Cpu6502 {
    registers: Registers,
    memory: Box<dyn Memory>,
}

impl CPU for Cpu6502 {
    fn reset(&mut self) -> Result<(), CpuError> {
        info!("resetting CPU");

        self.registers.a = 0;
        self.registers.x = 0;
        self.registers.y = 0;
        self.registers.p = StatusFlag::InterruptDisable.bits();
        self.registers.sp = 0xFD;
        self.registers.pc = 0xFFFC;
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), CpuError> {
        info!("initializing CPU");

        self.reset()?;
        self.memory.initialize()?;
        Ok(())
    }

    fn panic(&self, error: &CpuError) {
        error!("fatal exception: {}", error);
        self.dump_registers();
        self.dump_flags();
    }

    fn dump_registers(&self) {
        info!("CPU registers dump:");
        info!("- P (status): {:08b}", self.registers.p);
        info!("- A (accumulator): 0x{:02x}", self.registers.a);
        info!("- X (index): 0x{:02x}", self.registers.x);
        info!("- Y (index): 0x{:02x}", self.registers.y);
        info!("- SP (stack pointer): 0x{:02x}", self.registers.sp);
        info!("- PC (program counter): 0x{:04x}", self.registers.pc);
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

    fn run(&mut self) -> Result<(), CpuError> {
        info!("running CPU ...");

        loop {
            debug!("pc: 0x{:04x}", self.registers.pc);

            let byte = self.memory.read_byte(self.registers.pc)?;
            let instruction = Cpu6502::decode_instruction(byte)?;
            let operand = self.fetch_operand(instruction)?;

            self.execute_instruction(instruction, &operand)?;
            sleep(Duration::from_millis(200));
        }
    }
}

impl Cpu6502 {
    pub fn new(memory: Box<dyn Memory>) -> Self {
        Cpu6502 {
            registers: Registers {
                a: 0,
                x: 0,
                y: 0,
                p: 0,
                sp: 0,
                pc: 0
            },
            memory
        }
    }

    fn is_valid_stack_addr(&self, addr: u16) -> Result<(), CpuError> {
        if addr > STACK_END_ADDRESS {
            Err(CpuError::StackOverflow)
        } else if addr < STACK_BASE_ADDRESS {
            Err(CpuError::StackUnderflow)
        } else {
            Ok(())
        }
    }

    fn push_stack(&mut self, value: u8) -> Result<(), CpuError> {
        let mut addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
        self.memory.write_byte(addr, value)?;

        addr = addr - 1;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        Ok(())
    }

    fn pop_stack(&mut self) -> Result<u8, CpuError> {
        let addr = STACK_BASE_ADDRESS + self.registers.sp as u16 + 1;
        self.is_valid_stack_addr(addr)?;

        let value = self.memory.read_byte(addr)?;

        self.registers.sp = self.registers.sp | addr as u8;
        Ok(value)
    }

    fn safe_pc_add(&self, n: u16) -> Result<u16, CpuError> {
        let pc = self.registers.pc;
        let pc = pc.checked_add(n)
            .ok_or(MemoryError::OutOfBounds(self.registers.pc))?;

        Ok(pc)
    }

    fn safe_pc_sub(&self, n: u16) -> Result<u16, CpuError> {
        let pc = self.registers.pc;
        let pc = pc.checked_sub(n)
            .ok_or(MemoryError::OutOfBounds(self.registers.pc))?;

        Ok(pc)
    }

    fn decode_instruction<'a>(byte: u8) -> Result<&'a Instruction, CpuError> {
        let aaa = (byte & 0b1110_0000) >> 2;
        let cc = byte & 0b0000_0011;
        let opcode = aaa | cc;

        debug!("decoded instruction: raw: 0x{:02X} => opcode: 0x{:02X}", byte, opcode);

        if let Some(instruction) = INSTRUCTIONS_TABLE.get(&opcode) {
            Ok(instruction)
        } else {
            Err(CpuError::InvalidOpcode(byte))
        }
    }

    fn fetch_operand(&self, instruction: &Instruction) -> Result<Option<Value>, CpuError> {

        debug!("fetching operand for instruction: {:?}, {:?}", instruction.opcode, instruction.addressing_mode);

        let operand = match instruction.addressing_mode {
            AddressingMode::Implicit => {
                None
            },

            AddressingMode::Accumulator => {
                Some(Value::Accumulator)
            },

            AddressingMode::Immediate => {
                let pc = self.safe_pc_add(1)?;
                Some(Value::Byte(self.memory.read_byte(pc)?))
            },

            AddressingMode::Absolute => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                Some(Value::Byte(self.memory.read_byte(addr)?))
            },

            AddressingMode::AbsoluteIndexedX => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                let indexed_addr = addr + (self.registers.x as u16);
                Some(Value::Byte(self.memory.read_byte(indexed_addr)?))
            }

            AddressingMode::AbsoluteIndexedY => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_word(pc)?;
                let indexed_addr = addr + (self.registers.y as u16);
                Some(Value::Byte(self.memory.read_byte(indexed_addr)?))
            }


            AddressingMode::ZeroPage => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                Some(Value::Byte(self.memory.read_byte(addr as u16)?))
            },

            AddressingMode::ZeroPageIndexedX => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indexed_addr = addr + (self.registers.x);
                Some(Value::Byte(self.memory.read_byte(indexed_addr as u16)?))
            },

            AddressingMode::ZeroPageIndexedY => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indexed_addr = addr + (self.registers.y);
                Some(Value::Byte(self.memory.read_byte(indexed_addr as u16)?))
            },

            AddressingMode::Indirect => {
                let pc = self.safe_pc_add(1)?;
                let indirect_addr = self.memory.read_word(pc)?;
                Some(Value::Word(self.memory.read_word(indirect_addr)?))
            },

            AddressingMode::IndirectIndexedX => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indirect_addr = addr + self.registers.x;
                Some(Value::Byte(self.memory.read_byte(indirect_addr as u16)?))
            },

            AddressingMode::IndirectIndexedY => {
                let pc = self.safe_pc_add(1)?;
                let addr = self.memory.read_byte(pc)?;
                let indirect_addr = self.memory.read_word(addr as u16)? + self.registers.y as u16;
                Some(Value::Byte(self.memory.read_byte(indirect_addr)?))
            },

            AddressingMode::Relative => {
                let pc = self.safe_pc_add(1)?;
                let offset = self.memory.read_byte(pc)? as i8;
                let addr = if offset < 0 {
                    self.safe_pc_sub(offset.abs() as u16)?
                } else {
                    self.safe_pc_add(offset as u16)?
                };

                Some(Value::Word(self.memory.read_word(addr)?))
            },
        };

        debug!("fetched operand: {:?}", operand);
        Ok(operand)
    }

    fn execute_instruction(&mut self, instruction: &Instruction, operand: &Option<Value>) -> Result<(), CpuError> {
        debug!("executing instruction: opcode: {:?}, addressing mode: {:?}, operand: {:?}", instruction.opcode, instruction.addressing_mode, operand);
        (instruction.execute)(instruction, self, operand)
    }
}

impl Instruction {

    fn brk_force_interrupt(&self, cpu: &mut Cpu6502, _: &Option<Value>) -> Result<(), CpuError> {
        cpu.registers.p |= StatusFlag::BreakCommand.bits();

        let next_pc = cpu.safe_pc_add(2)?;

        cpu.push_stack((next_pc >> 8) as u8)?;
        cpu.push_stack((next_pc & 0xFF) as u8)?;
        cpu.push_stack(cpu.registers.p)?;

        cpu.registers.set_status(StatusFlag::InterruptDisable, true);

        cpu.registers.pc = cpu.memory.read_word(0xFFFE)?;

        Ok(())
    }

    fn adc_add_memory_to_accumulator_with_carry(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            let carry_flag = cpu.registers.get_status(StatusFlag::Carry);
            let result = cpu.registers.a as u16 + *value as u16 + carry_flag as u16;

            cpu.registers.a = result as u8;

            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80!= 0);
            cpu.registers.set_status(StatusFlag::Carry, result > 0xFF);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn and_and_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            cpu.registers.a = cpu.registers.a & value;

            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn asl_shift_left_one_bit(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {

        let (original_value, result) = match operand {
            Some(Value::Accumulator) => {
                let original_value = cpu.registers.a;
                let result = original_value << 1;
                cpu.registers.a = result;
                (original_value, result)
            },

            Some(Value::Word(addr)) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let result = original_value << 1;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },

            _ => {
                return Err(CpuError::InvalidOperand(format!("{:?}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        Ok(())
    }

    fn bcc_branch_on_carry_clear(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bcs_branch_on_carry_set(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn beq_branch_on_result_zero(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bit_test_bits_in_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bmi_branch_on_result_minus(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bne_branch_on_result_not_zero(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bpl_branch_on_result_plus(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn brk_force_break(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        cpu.registers.p |= StatusFlag::BreakCommand.bits();

        let next_pc = cpu.safe_pc_add(2)?;

        cpu.push_stack((next_pc >> 8) as u8)?;
        cpu.push_stack((next_pc & 0xFF) as u8)?;
        cpu.push_stack(cpu.registers.p)?;

        cpu.registers.p |= StatusFlag::InterruptDisable.bits();
        cpu.registers.pc = cpu.memory.read_word(0xFFFE)?;

        Ok(())
    }

    fn bvc_branch_on_overflow_clear(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn bvs_branch_on_overflow_set(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn clc_clear_carry_flag(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn cld_clear_decimal_mode(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn cli_clear_interrupt_disable_bit(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn clv_clear_overflow_flag(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn cmp_compare_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            let result = cpu.registers.a.wrapping_sub(*value);

            cpu.registers.set_status(StatusFlag::Carry, cpu.registers.a > *value);
            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn cpx_compare_memory_and_index_x(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn cpy_compare_memory_and_index_y(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn dec_decrement_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Word(addr)) = operand {
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

    fn dex_decrement_index_x_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn dey_decrement_index_y_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn eor_exclusive_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            cpu.registers.a = cpu.registers.a ^ value;

            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn inc_increment_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Word(addr)) = operand {
            let mut value = cpu.memory.read_byte(*addr)?;

            value = value.wrapping_add(1);
            cpu.memory.write_byte(*addr, value)?;

            cpu.registers.set_status(StatusFlag::Zero, value == 0);
            cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("Expected a memory address, got: {:?}", operand)))
        }
    }

    fn inx_increment_index_x_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn iny_increment_index_y_by_one(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn jmp_jump_to_new_location(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn jsr_jump_to_new_location_saving_return_address(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn lda_load_accumulator_with_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            cpu.registers.a = *value;

            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn ldx_load_index_x_with_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Word(value)) = operand {
            cpu.registers.x = cpu.memory.read_byte(*value)?;
            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
            cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);
            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn ldy_load_index_y_with_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn lsr_shift_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Some(Value::Accumulator) => {
                let original_value = cpu.registers.a;
                let result = original_value >> 1;
                cpu.registers.a = result;
                (original_value, result)
            },
            Some(Value::Word(addr)) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let result = original_value >> 1;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },
            _ => {
                return Err(CpuError::InvalidOperand(format!("{:?}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, false);
        Ok(())
    }

    fn nop_no_operation(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn ora_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            cpu.registers.a = cpu.registers.a | value;
            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn pha_push_accumulator_on_stack(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn php_push_processor_status_on_stack(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn pla_pull_accumulator_from_stack(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn plp_pull_processor_status_from_stack(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn rol_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Some(Value::Accumulator) => {
                let original_value = cpu.registers.a;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };
                let result = (original_value << 1) | carry_in;
                cpu.registers.a = result;
                (original_value, result)
            },

            Some(Value::Word(addr)) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };
                let result = (original_value << 1) | carry_in;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },
            _ => {
                return Err(CpuError::InvalidOperand(format!("{:?}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        Ok(())
    }

    fn ror_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        let (original_value, result) = match operand {
            Some(Value::Accumulator) => {
                let original_value = cpu.registers.a;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
                let result = (original_value >> 1) | carry_in;
                cpu.registers.a = result;
                (original_value, result)
            },
            Some(Value::Word(addr)) => {
                let original_value = cpu.memory.read_byte(*addr)?;
                let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
                let result = (original_value >> 1) | carry_in;
                cpu.memory.write_byte(*addr, result)?;
                (original_value, result)
            },
            _ => {
                return Err(CpuError::InvalidOperand(format!("{:?}", operand)));
            }
        };

        cpu.registers.set_status(StatusFlag::Carry, original_value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        Ok(())
    }

    fn rti_return_from_interrupt(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn rts_return_from_subroutine(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn sbc_subtract_memory_from_accumulator_with_borrow(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Byte(value)) = operand {
            let borrow = if cpu.registers.get_status(StatusFlag::Carry) { 0 } else { 1 };
            let result = cpu.registers.a.wrapping_sub(*value).wrapping_sub(borrow);
            let carry = cpu.registers.a >= *value + borrow;
            let overflow = ((cpu.registers.a ^ result) & 0x80 != 0) && ((cpu.registers.a ^ *value) & 0x80 == 0);

            cpu.registers.a = result;

            cpu.registers.set_status(StatusFlag::Carry, carry);
            cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
            cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
            cpu.registers.set_status(StatusFlag::Overflow, overflow);

            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn sec_set_carry_flag(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn sed_set_decimal_flag(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn sei_set_interrupt_disable_status(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn sta_store_accumulator_in_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Word(value)) = operand {
            cpu.memory.write_byte(*value, cpu.registers.a)?;
            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn stx_store_index_x_in_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        if let Some(Value::Word(value)) = operand {
            cpu.memory.write_byte(*value, cpu.registers.x)?;
            Ok(())
        } else {
            Err(CpuError::InvalidOperand(format!("{:?}", operand)))
        }
    }

    fn sty_store_index_y_in_memory(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn tax_transfer_accumulator_to_index_x(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn tay_transfer_accumulator_to_index_y(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn tsx_transfer_stack_pointer_to_index_x(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn txa_transfer_index_x_to_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn txs_transfer_index_x_to_stack_register(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }

    fn tya_transfer_index_y_to_accumulator(&self, cpu: &mut Cpu6502, operand: &Option<Value>) -> Result<(), CpuError> {
        Err(CpuError::UnImplemented(format!("{:?}", self.opcode)))
    }


}