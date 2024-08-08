use std::collections::HashMap;
use std::thread::sleep;
use std::time::Duration;
use log::{debug, info};
use once_cell::sync::Lazy;
use crate::cpu::{CPU, CpuError};
use crate::memory::{Memory, MemoryError};

const CLOCK_HZ: usize = 1_789_773;

const STACK_BASE_ADDRESS: u16 = 0x0100;
const STACK_END_ADDRESS: u16 = 0x01FF;

#[derive(Debug)]
enum Value {
    Byte(u8),
    Word(u16),
}

pub static INSTRUCTIONS_TABLE: Lazy<HashMap<u8, Instruction>> = Lazy::new(|| {
    let mut map = HashMap::<u8, Instruction>::new();

    map.insert(0x00, Instruction {
        opcode: OpCode::BRK,
        addressing_mode: AddressingMode::Implicit,
        bytes: 1,
        cycles: 7,
        execute: Cpu6502::brk_force_interrupt
    });
    map
});

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
    execute: fn(&mut Cpu6502, &Option<Value>) -> Result<(), CpuError>,
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
                Some(Value::Byte(self.registers.a))
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
                let addr = if (offset < 0) {
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
        debug!("executing instruction: opcode: {:?}, addressing mode: {:?}", instruction.opcode, instruction.addressing_mode);
        (instruction.execute)(self, operand)
    }

    fn brk_force_interrupt(cpu: &mut Cpu6502, _: &Option<Value>) -> Result<(), CpuError> {
        cpu.registers.p |= StatusFlag::BreakCommand.bits();

        let next_pc = cpu.safe_pc_add(2)?;

        cpu.push_stack((next_pc >> 8) as u8)?;
        cpu.push_stack((next_pc & 0xFF) as u8)?;
        cpu.push_stack(cpu.registers.p)?;

        cpu.registers.p |= StatusFlag::InterruptDisable.bits();
        cpu.registers.pc = cpu.memory.read_word(0xFFFE)?;

        Ok(())
    }

    fn ora_logical_inclusive_or(cpu: &mut Cpu6502, operand: &Option<Value>) {
        //cpu.registers.a |= operand;
    }
}