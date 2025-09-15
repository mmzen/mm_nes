use std::{fmt, io};
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::fs::File;
use std::io::Write;
use std::rc::Rc;
use std::sync::Arc;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use crate::bus::Bus;
use crate::cpu::{CPU, CpuError, Interruptible};
use crate::cpu_debugger::{Breakpoints, CpuSnapshot};
use crate::memory::{MemoryError};

//const CLOCK_HZ: usize = 1_789_773;
const STACK_BASE_ADDRESS: u16 = 0x0100;
const STACK_END_ADDRESS: u16 = 0x01FF;

const IRQ_VECTOR: u16 = 0xFFFE;
const NMI_VECTOR: u16 = 0xFFFA;
const BRK_VECTOR: u16 = 0xFFFE;
const RESET_VECTOR: u16 = 0xFFFC;
const NUM_OP_CODES: usize = 256;

static INSTRUCTION_TABLE: Lazy<Vec<Instruction>> = Lazy::new(|| {
    Cpu6502::build_instruction_table()
});

macro_rules! add_instruction {
            ($table:ident, $opcode:expr, $op:ident, $addr_mode:ident, $bytes:expr, $cycles:expr, $exec:ident, $category:ident) => {
                $table[$opcode] = Instruction {
                    opcode: OpCode::$op,
                    addressing_mode: AddressingMode::$addr_mode,
                    bytes: $bytes,
                    cycles: $cycles,
                    execute: Instruction::$exec,
                    category: InstructionCategory::$category
                }
            };
        }

#[derive(Debug, Clone, Copy)]
enum OpCode {
    ADC, ALR, ANC, AND, ANE, ARR, ASL, BCC, BCS, BEQ, BIT, BMI, BNE, BPL, BRK, BVC, BVS, CLC, CLD,
    CLI, CLV, CMP, CPX, CPY, DCP, DEC, DEX, DEY, EOR, INC, INX, INY, ISB, JAM, JMP, JSR, LAS, LAX,
    LDA, LDX, LDY, LSR, LXA, NOP, ORA, PHA, PHP, PLA, PLP, RLA, ROL, ROR, RRA, RTI, RTS, SAX, SBC,
    SBX, SEC, SED, SEI, SHA, SHX, SHY, SLO, SRE, STA, STX, STY, TAX, TAY, TSX, TXA, TXS, TYA, XXX
}

impl Display for OpCode {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Debug, Clone)]
enum Operand {
    Byte(u8),
    Address(u16),
    AddressAndEffectiveAddress(u16, u16, bool),
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
            Operand::AddressAndEffectiveAddress(addr, effective, page_crossed) => {
                write!(f, "address: 0x{:04X}, effective address: 0x{:04X}, page crossed: {}", addr, effective, page_crossed)
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
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

#[derive(Debug, PartialEq, Clone, Copy)]
enum InstructionCategory {
    Standard,
    Illegal
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

#[derive(Debug, Clone)]
struct Registers  {
    a: u8,      // Accumulator register
    x: u8,      // Index register X
    y: u8,      // Index register Y
    p: u8,      // Status register
    sp: u8,     // Stack pointer
    pc: u16,    // Program counter
    is_pc_dirty: bool // Indicates whether the program counter needs to be updated
}

impl Registers {

    fn set_status(&mut self, flag: StatusFlag, value: bool) {
        if value {
            //debug!("CPU: setting status flag: {:?}, {:04X}", flag, flag.bits());
            self.p |= flag.bits();
        } else {
            //debug!("CPU: clearing status flag: {:?}, {:04X}", flag, flag.bits());
            self.p &= !flag.bits();
        }
    }

    fn get_status(&self, flag: StatusFlag) -> bool {
        //debug!("CPU: status flag: {:?}, {:04X}", flag, flag.bits());
        (self.p & flag.bits()) != 0
    }

    fn safe_pc_add(&self, n: i16) -> Result<u16, CpuError> {
        let pc = self.pc;

        let pc = if n >= 0 {
            pc.wrapping_add(n as u16)
        } else {
            pc.wrapping_sub((-n) as u16)
        };

        Ok(pc)
    }

    fn set_pc(&mut self, pc: u16) {
        self.pc = pc;
        self.is_pc_dirty = true;
    }
}

pub const APU_FRAME_COUNTER_IRQ: u8 = 0x01;
pub const APU_DMC_IRQ: u8 = 0x02;
pub const PPU_NMI: u8 = 0x80;

#[derive(Debug, Default)]
struct InterruptMask(u8);

impl InterruptMask {
    fn set(&mut self, mask: u8) {
        self.0 |= mask;
    }

    fn unset(&mut self, mask: u8) {
        self.0 &= !mask;
    }

    fn is_set(&self, mask: u8) -> bool {
        (self.0 & mask) == mask
    }

    fn has_irq(&self) -> bool {
        (self.0 & !PPU_NMI) != 0
    }

    fn has_nmi(&self) -> bool {
        self.0 & PPU_NMI != 0
    }
}

#[derive(Clone, Debug)]
struct Cpu6502Snapshot {
    registers: Registers,
    instruction: Vec<u8>,
    mnemonic: String,
    is_illegal: bool,
    operand: String,
    cycles: u32,
}

impl CpuSnapshot for Cpu6502Snapshot {
    fn pc(&self) -> u16 { self.registers.pc }
    fn a(&self) -> u8 { self.registers.a }
    fn x(&self) -> u8 { self.registers.x }
    fn y(&self) -> u8 { self.registers.y }
    fn sp(&self) -> u8 { self.registers.sp }
    fn p(&self) -> u8 { self.registers.p }

    fn instruction(&self) -> Vec<u8> {
        self.instruction.clone()
    }

    fn mnemonic(&self) -> String {
        self.mnemonic.clone()
    }

    fn is_illegal(&self) -> bool {
        self.is_illegal
    }

    fn operand(&self) -> String {
        self.operand.clone()
    }

    fn cycles(&self) -> u32 {
        self.cycles
    }
}

impl Cpu6502Snapshot {
    
    fn new(registers: Registers, bus: Rc<RefCell<dyn Bus>>, cycles: u32) -> Result<Cpu6502Snapshot, CpuError> {
        let byte = bus.borrow().read_byte(registers.pc)?;
        let instr0 = Cpu6502::decode_instruction(byte)?;
        let instruction = Cpu6502Snapshot::build_instruction(instr0, &registers, bus.clone())?;
        let mnemonic = instr0.opcode.to_string();
        let oper0 = Cpu6502::fetch_operand(instr0, &registers, bus.clone())?;
        let operand = Cpu6502Snapshot::build_operand(&instr0, &oper0, &registers, bus.clone())?;
        
        let snapshot = Cpu6502Snapshot {
            registers,
            instruction,
            mnemonic,
            is_illegal: instr0.category == InstructionCategory::Illegal,
            operand,
            cycles,
        };
        
        Ok(snapshot)
    }
    
    fn build_instruction(instruction: &Instruction, registers: &Registers, bus: Rc<RefCell<dyn Bus>>) -> Result<Vec<u8>, CpuError> {
        let mut bytes = Vec::<u8>::new();
        let byte0 = bus.borrow().trace_read_byte(registers.pc)?;
        bytes.push(byte0);

        for i in 1..instruction.bytes {
            let byte = bus.borrow().trace_read_byte(registers.safe_pc_add(i as i16)?)?;
            bytes.push(byte);
        }

        Ok(bytes)
    }
    
    fn build_operand(instruction: &Instruction, operand: &Operand, registers: &Registers, bus: Rc<RefCell<dyn Bus>>) -> Result<String, CpuError> {
        let operand = match (instruction.addressing_mode, operand, instruction.opcode) {
            (AddressingMode::Implicit, _, _) =>
                "".to_string(),

            (AddressingMode::Accumulator, _, _) =>
                "A".to_string(),

            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JMP) |
            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JSR) =>
                format!("${:04X}", *addr),

            (AddressingMode::Absolute, Operand::Address(addr), _) =>
                format!("${:04X} = {:02X}", *addr, bus.borrow().trace_read_byte(*addr)?),

            (AddressingMode::Relative, Operand::Address(addr), _) =>
                format!("${:04X}", *addr),

            (AddressingMode::ZeroPage, Operand::Address(addr), _) =>
                format!("${:02X} = {:02X}", *addr as u8, bus.borrow().trace_read_byte(*addr)?),

            (AddressingMode::AbsoluteIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},X @ {:04X} = {:02X}", *addr, *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::AbsoluteIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},Y @ {:04X} = {:02X}", *addr, *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},X @ {:02X} = {:02X}", *addr, *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},Y @ {:02X} = {:02X}", *addr, *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::Indirect, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:04X}) = {:04X}", *addr, *effective),

            (AddressingMode::IndirectIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X},X) @ {:02X} = {:04X} = {:02X}", *addr, (*addr as u8).wrapping_add(registers.x),
                        *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::IndirectIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X}),Y = {:04X} @ {:04X} = {:02X}", *addr, (*effective).wrapping_sub(registers.y as u16),
                        *effective, bus.borrow().trace_read_byte(*effective)?),

            (AddressingMode::Immediate, Operand::Byte(byte), _) =>
                format!("#${:02X}", byte),

            _ => {
                return Err(CpuError::InvalidOperand(
                    format!("could not format instruction and operand: {:?}, {:?}", instruction.addressing_mode, operand)
                ))
            }
        };

        Ok(operand)
    }
}

#[derive(Debug)]
pub struct Cpu6502 {
    registers: Registers,
    bus: Rc<RefCell<dyn Bus>>,
    instructions_executed: u64,
    interrupt: InterruptMask,
    cycles: u32,
}

impl Interruptible for Cpu6502 {
    fn signal_irq(&mut self, irq_source: u8) -> Result<(), CpuError> {
        self.interrupt.set(irq_source);
        Ok(())
    }

    fn clear_irq(&mut self, irq_source: u8) -> Result<(), CpuError> {
        if self.interrupt.is_set(irq_source) {
            self.interrupt.unset(irq_source);
        }
        Ok(())
    }

    fn is_asserted_irq(&self) -> Result<bool, CpuError> {
        Ok(self.interrupt.has_irq())
    }

    fn is_asserted_irq_by_source(&self, irq_source: u8) -> Result<bool, CpuError> {
        Ok(self.interrupt.is_set(irq_source))
    }

    fn signal_nmi(&mut self) -> Result<(), CpuError> {
        self.interrupt.set(PPU_NMI);
        Ok(())
    }

    fn clear_nmi(&mut self) -> Result<(), CpuError> {
        self.interrupt.unset(PPU_NMI);
        Ok(())
    }

    fn is_asserted_nmi(&self) -> Result<bool, CpuError> {
        Ok(self.interrupt.has_nmi())
    }
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
        self.set_pc_indirect(RESET_VECTOR)?;

        Ok(())
    }

    fn initialize(&mut self) -> Result<(), CpuError> {
        info!("initializing CPU");
        self.reset()?;
        Ok(())
    }

    fn panic(&self, error: &CpuError) {
        error!("fatal exception: {}", error);
        self.dump_registers();
        self.dump_flags();
        //self.dump_memory();
        info!("number of instructions executed: {}", self.instructions_executed);
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
        self.bus.borrow().dump();
    }

    fn step_instruction(&mut self) -> Result<u32, CpuError> {
        let byte = self.bus.borrow().read_byte(self.registers.pc)?;
        let instruction = Cpu6502::decode_instruction(byte)?;
        let operand = Cpu6502::fetch_operand(instruction, &self.registers, self.bus.clone())?;

        let additional_cycles = self.execute_instruction(&instruction, &operand)?;
        let cycles = instruction.cycles + additional_cycles;

        if self.registers.is_pc_dirty == false {
            self.registers.pc = self.registers.safe_pc_add(instruction.bytes as i16)?;
        } else {
            self.registers.is_pc_dirty = false;
        }

        self.instructions_executed += 1;
        self.cycles += cycles;

        self.interrupt()?;  // some additional cycles are probably needed here (7?)

        Ok(cycles)
    }

    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, CpuError> {
        let mut cycles = start_cycle;
        let cycles_threshold = start_cycle + credits;

        loop {
            cycles += self.step_instruction_with_cycles(start_cycle)?;

            if cycles >= cycles_threshold {
                break;
            }
        }

        Ok(cycles)
    }

    fn run_until_breakpoint(&mut self, start_cycle: u32, credits: u32, breakpoints: Box<dyn Breakpoints>) -> Result<(u32, bool), CpuError> {
        let mut cycles = start_cycle;
        let cycles_threshold = start_cycle + credits;

        loop {
            if breakpoints.contains(self.registers.pc) {
                return Ok((cycles, true));
            } else {
                cycles +=self.step_instruction_with_cycles(start_cycle)?;

                if cycles >= cycles_threshold {
                    break;
                }
            }
        }

        Ok((cycles, false))
    }

    fn set_pc_immediate(&mut self, address: u16) -> Result<(), CpuError> {
        self.registers.pc = address;
        //debug!("CPU: pc set to effective address 0x{:04X}", self.registers.pc);

        Ok(())
    }

    fn set_pc_indirect(&mut self, address: u16) -> Result<(), CpuError> {
        self.registers.pc = self.bus.borrow().read_word(address)?;
        //debug!("CPU: pc set to effective address 0x{:04X} (address: 0x{:04X})",
        //    self.registers.pc, address);

        Ok(())
    }

    fn snapshot(&self) -> Result<Box<dyn CpuSnapshot>, CpuError> {
        let registers = Registers {
            a: self.registers.a,
            x: self.registers.x,
            y: self.registers.y,
            p: self.registers.p,
            sp: self.registers.sp,
            pc: self.registers.pc,
            is_pc_dirty: false,
        };
        
        let snapshot = Cpu6502Snapshot::new(registers, self.bus.clone(), self.cycles)?;
        Ok(Box::new(snapshot))
    }
}

impl Cpu6502 {
    pub fn new(bus: Rc<RefCell<dyn Bus>>) -> Self {
        Cpu6502 {
            registers: Registers {
                a: 0,
                x: 0,
                y: 0,
                p: 0,
                sp: 0,
                pc: 0,
                is_pc_dirty: false,
            },
            bus,
            instructions_executed: 0,
            interrupt: InterruptMask::default(),
            cycles: 0,
        }
    }

    fn interrupt(&mut self) -> Result<(), CpuError> {
        if self.is_asserted_nmi()? {
            self.nmi()?;
            self.clear_nmi()?;
        } else if self.is_asserted_irq()? && !self.registers.get_status(StatusFlag::InterruptDisable) {
            self.irq()?;
        }

        Ok(())
    }

    fn build_instruction_table() -> Vec<Instruction> {
        let illegal_instruction = Instruction {
            opcode: OpCode::XXX,
            addressing_mode: AddressingMode::Implicit,
            bytes: 1,
            cycles: 1,
            execute: Instruction::illegal,
            category: InstructionCategory::Standard
        };

        let mut table = [illegal_instruction; NUM_OP_CODES].to_vec();
        include!("instructions_macro_all.rs");

        //debug!("CPU: dumping instruction table:");
        //for (index, p) in table.iter().enumerate() {
        //debug!("CPU:    0x{:02X}: {:?}, {} bytes, {} cycles", index, p.opcode, p.bytes, p.cycles);
        //}

        table
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

    fn stack_wrapping_add(addr: u16, value: i8) -> u16 {
        let sp = addr as u8;
        STACK_BASE_ADDRESS + sp.wrapping_add_signed(value) as u16
    }

    fn push_stack(&mut self, value: u8) -> Result<(), CpuError> {
        let mut addr = STACK_BASE_ADDRESS + self.registers.sp as u16;

        //debug!("CPU: sp (before push): 0x{:02X}, pushing at 0x{:04X}, value {:04X}", self.registers.sp, addr, value);
        self.bus.borrow_mut().write_byte(addr, value)?;

        addr = Cpu6502::stack_wrapping_add(addr, -1);
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        //debug!("CPU: sp (after push): 0x{:02X}", self.registers.sp);

        Ok(())
    }

    fn pop_stack(&mut self) -> Result<u8, CpuError> {
        let mut addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
        addr = Cpu6502::stack_wrapping_add(addr, 1);

        //debug!("CPU: sp (before pop): 0x{:02X}, popping at 0x{:04X}", self.registers.sp, addr);
        let value = self.bus.borrow().read_byte(addr)?;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        //debug!("CPU: sp (after pop): 0x{:02X}, popped value {:02X}", self.registers.sp, value);

        Ok(value)
    }

    fn read_word_with_page_wrap(addr: u16, bus: Rc<RefCell<dyn Bus>>) -> Result<u16, MemoryError> {
        let lo = bus.borrow().read_byte(addr)?;

        let hi = if (addr & 0xFF) == 0xFF {
            bus.borrow().read_byte(addr & 0xFF00)?
        } else {
            bus.borrow().read_byte(addr.wrapping_add(1))?
        };

        Ok((hi as u16) << 8 | lo as u16)
    }

    fn update_flags_zero_negative(&mut self, value: u8) {
        self.registers.set_status(StatusFlag::Zero, value == 0);
        self.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);
    }

    fn shift_left_and_update_carry_flags(&mut self, value: u8) -> u8 {
        let original_value = value;
        let result = value << 1;

        self.registers.set_status(StatusFlag::Carry, original_value & 0x80 != 0);
        self.update_flags_zero_negative(result);

        result
    }

    fn shift_right_and_update_carry_flags(&mut self, value: u8) -> u8 {
        let original_value = value;
        let result = value >> 1;

        self.registers.set_status(StatusFlag::Carry, original_value & 0x01 != 0);
        self.update_flags_zero_negative(result);

        result
    }

    fn overwrite(&mut self, operand: &Operand, value: u8) -> Result<(), CpuError> {
        match operand {
            Operand::Address(addr) |
            Operand::AddressAndEffectiveAddress(_, addr, _) => {
                self.bus.borrow_mut().write_byte(*addr, value)?;
                Ok(())
            },

            Operand::Accumulator => {
                self.registers.a = value;
                Ok(())
            },

            _ => {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        }
    }

    fn get_operand_byte_value(&self, operand: &Operand) -> Result<u8, CpuError> {
        let value = match operand {
            Operand::Accumulator => {
                Ok(self.registers.a)
            },
            Operand::Byte(value) => {
                Ok(*value)
            },
            Operand::Address(addr) => {
                let value = self.bus.borrow().read_byte(*addr)?;
                Ok(value)
            },
            Operand::AddressAndEffectiveAddress(_, effective, _) => {
                let value = self.bus.borrow().read_byte(*effective)?;
                Ok(value)
            }
            Operand::None => {
                Err(CpuError::InvalidOperand(format!("{}", operand)))
            }
        };

        value
    }

    fn get_operand_word_value(&self, operand: &Operand) -> Result<u16, CpuError> {
        let value = match operand {
            Operand::Address(addr) => {
                let value = *addr;
                Ok(value)
            },
            Operand::AddressAndEffectiveAddress(_, effective, _) => {
                let value = *effective;
                Ok(value)
            },
            _ => Err(CpuError::InvalidOperand(format!("{}", operand)))
        };

        value
    }

    fn is_page_crossed(addr1: u16, addr2: u16) -> bool {
        let page1 = addr1 & 0xFF00;
        let page2 = addr2 & 0xFF00;

        page1 != page2
    }

    fn get_cycles_by_page_crossing_for_conditional_jump(&self, source: u16, destination: u16) -> u32 {
        if Cpu6502::is_page_crossed(source, destination) { 2 } else { 1 }
    }

    fn get_cycles_by_page_crossing_for_load(&self, operand: &Operand) -> u32 {
        match operand {
            Operand::AddressAndEffectiveAddress(_, _, page_crossed) => {
                if *page_crossed { 1 } else { 0 }
            },
            _ => 0
        }
    }

    fn decode_instruction<'a>(byte: u8) -> Result<&'a Instruction, CpuError> {
        //let opcode = aaa | cc;
        //debug!("CPU: decoded instruction: 0x{:02X}: opcode: 0x{:02X}", byte, opcode);

        let instruction = &INSTRUCTION_TABLE[byte as usize];
        Ok(instruction)
    }

    fn fetch_operand(instruction: &Instruction, registers: &Registers, bus: Rc<RefCell<dyn Bus>>) -> Result<Operand, CpuError> {

        //debug!("CPU: fetching operand for instruction: {:?}, {:?}", instruction.opcode, instruction.addressing_mode);

        let operand = match instruction.addressing_mode {
            AddressingMode::Implicit => {
                Operand::None
            },

            AddressingMode::Accumulator => {
                Operand::Accumulator
            },

            AddressingMode::Immediate => {
                let pc = registers.safe_pc_add(1)?;

                Operand::Byte(bus.borrow().read_byte(pc)?)
            },

            AddressingMode::Absolute => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_word(pc)?;

                Operand::Address(addr)
            },

            AddressingMode::AbsoluteIndexedX => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_word(pc)?;
                let effective_addr = addr.wrapping_add(registers.x as u16);
                let page_crossed = Cpu6502::is_page_crossed(addr, effective_addr);

                Operand::AddressAndEffectiveAddress(addr, effective_addr, page_crossed)
            }

            AddressingMode::AbsoluteIndexedY => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_word(pc)?;
                let effective_addr = addr.wrapping_add(registers.y as u16);
                let page_crossed = Cpu6502::is_page_crossed(addr, effective_addr);

                Operand::AddressAndEffectiveAddress(addr, effective_addr, page_crossed)
            }

            AddressingMode::ZeroPage => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;

                Operand::Address(addr as u16)
            },

            AddressingMode::ZeroPageIndexedX => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;
                let effective_addr = addr.wrapping_add(registers.x) as u16;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, false)
            },

            AddressingMode::ZeroPageIndexedY => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;
                let effective_addr = addr.wrapping_add(registers.y) as u16;
                let page_crossed = Cpu6502::is_page_crossed(addr as u16, effective_addr);

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, page_crossed)
            },

            AddressingMode::Indirect => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_word(pc)?;
                let effective_addr = Cpu6502::read_word_with_page_wrap(addr, bus)?;

                Operand::AddressAndEffectiveAddress(addr, effective_addr, false)
            },

            AddressingMode::IndirectIndexedX => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;
                let indirect_addr = addr.wrapping_add(registers.x);
                let effective_addr = Cpu6502::read_word_with_page_wrap(indirect_addr as u16, bus)?;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, false)
            },

            AddressingMode::IndirectIndexedY => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;
                let indirect_addr = Cpu6502::read_word_with_page_wrap(addr as u16, bus)?;
                let effective_addr = indirect_addr.wrapping_add(registers.y as u16);
                let page_crossed = Cpu6502::is_page_crossed(indirect_addr, effective_addr);

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, page_crossed)
            },

            AddressingMode::Relative => {
                let pc = registers.safe_pc_add(1)?;
                let offset = bus.borrow().read_byte(pc)? as i8;
                let addr = registers.safe_pc_add(2)?;
                let addr= addr.wrapping_add_signed(offset as i16);

                Operand::Address(addr)
            },
        };

        //debug!("CPU: fetched operand: {}", operand);
        Ok(operand)
    }

    fn step_instruction_with_cycles(&mut self, start_cycle: u32) -> Result<u32, CpuError> {
        let cycles = self.step_instruction()?;
        self.cycles = start_cycle + cycles;

        Ok(cycles)
    }

    fn execute_instruction(&mut self, instruction: &Instruction, operand: &Operand) -> Result<u32, CpuError> {

        //debug!("CPU: executing instruction: opcode: {:?}, addressing mode: {:?}, operand: {}",
        //    instruction.opcode, instruction.addressing_mode, operand);

        (instruction.execute)(instruction, self, operand)
    }

    fn interrupt_preamble(&mut self) -> Result<(), CpuError> {
        self.registers.p &= !StatusFlag::BreakCommand.bits();

        self.push_stack((self.registers.pc >> 8) as u8)?;
        self.push_stack((self.registers.pc & 0xFF) as u8)?;
        self.push_stack(self.registers.p)?;

        self.registers.set_status(StatusFlag::InterruptDisable, true);
        Ok(())
    }

    fn nmi(&mut self) -> Result<(), CpuError> {
        self.interrupt_preamble()?;
        self.registers.pc = self.bus.borrow().read_word(NMI_VECTOR)?;
        //debug!("CPU: NMI interrupt: program counter: 0x{:04X}", self.registers.pc);

        Ok(())
    }

    fn irq(&mut self) -> Result<(), CpuError> {
        self.interrupt_preamble()?;
        self.registers.pc = self.bus.borrow().read_word(IRQ_VECTOR)?;
        //debug!("CPU: IRQ interrupt: program counter: 0x{:04X}", self.registers.pc);

        Ok(())
    }

    #[cfg(test)]
    pub fn get_internal_interrupt_value(&self) -> u8 {
        self.interrupt.0
    }

    #[cfg(test)]
    pub fn clear_internal_interrupt_value(&mut self) {
        self.interrupt.0 = 0;
    }
}

#[derive(Clone, Copy, Debug)]
struct Instruction {
    opcode: OpCode,
    addressing_mode: AddressingMode,
    bytes: usize,
    cycles: u32,
    category: InstructionCategory,
    execute: fn(&Instruction, &mut Cpu6502, &Operand) -> Result<u32, CpuError>,
}

impl Instruction {

    fn adc_add_memory_to_accumulator_with_carry(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let carry = cpu.registers.get_status(StatusFlag::Carry) as u8;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        let a = cpu.registers.a;
        let (t0, overflow0) = a.overflowing_add(value);
        let (t1, overflow1) = t0.overflowing_add(carry);

        cpu.registers.a = t1;

        cpu.registers.set_status(StatusFlag::Carry, overflow0 | overflow1);
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        let overflow = ((a ^ cpu.registers.a) & 0x80 != 0) && ((a ^ value) & 0x80 == 0);
        cpu.registers.set_status(StatusFlag::Overflow, overflow);

        Ok(cycles)
    }

    fn and_and_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = cpu.registers.a & value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn asl_shift_left_one_bit(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result = cpu.shift_left_and_update_carry_flags(value);

        cpu.update_flags_zero_negative(result);
        cpu.overwrite(operand, result)?;

        Ok(0)
    }

    fn bcc_branch_on_carry_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Carry) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bcs_branch_on_carry_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Carry) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    /***
     * BEQ - Branch on Result Zero
     * it seems there is an issue: it is likely that pc get incremented after
     * returning from this function. It shall not happen, as pc is forced to a target value.
     * E755  F0 FE     BEQ $E755
     * returning to run() triggers an issue, as pc was modified to loop back to E755, but it is not
     * detected as the value is kept the same, and then pc is incremented
     * TODO: introduce a dirty flag when pc is touched.
     */
    fn beq_branch_on_result_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Zero) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bit_test_bits_in_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let value = cpu.bus.borrow().read_byte(addr)?;
        let result = cpu.registers.a & value;

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Overflow, value & 0x40 != 0);

        Ok(0)
    }

    fn bmi_branch_on_result_minus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Negative) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bne_branch_on_result_not_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Zero) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bpl_branch_on_result_plus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Negative) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn brk_force_break(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.p |= StatusFlag::BreakCommand.bits();

        let next_pc = cpu.registers.safe_pc_add(2)?;

        cpu.push_stack((next_pc >> 8) as u8)?;
        cpu.push_stack((next_pc & 0xFF) as u8)?;
        cpu.push_stack(cpu.registers.p)?;

        cpu.registers.set_status(StatusFlag::InterruptDisable, true);

        // XXX not sure about this
        let addr = if cpu.is_asserted_nmi()? {
            cpu.clear_nmi()?;
            cpu.bus.borrow().read_word(NMI_VECTOR)?
        } else if cpu.is_asserted_irq()? && !cpu.registers.get_status(StatusFlag::InterruptDisable) {
            cpu.bus.borrow().read_word(IRQ_VECTOR)?
        } else {
            cpu.bus.borrow().read_word(BRK_VECTOR)?
        };

        cpu.registers.set_pc(addr);

        Ok(0)
    }

    fn bvc_branch_on_overflow_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Overflow) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bvs_branch_on_overflow_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Overflow) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn clc_clear_carry_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::Carry, false);
        Ok(0)
    }

    fn cld_clear_decimal_mode(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::DecimalMode, false);
        Ok(0)
    }

    fn cli_clear_interrupt_disable_bit(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::InterruptDisable, false);
        Ok(0)
    }

    fn clv_clear_overflow_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::Overflow, false);
        Ok(0)
    }

    fn cmp_compare_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        let result = cpu.registers.a.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.a);

        Ok(cycles)
    }

    fn cpx_compare_memory_and_index_x(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;

        let result = cpu.registers.x.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.x);

        Ok(0)
    }

    fn cpy_compare_memory_and_index_y(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;

        let result = cpu.registers.y.wrapping_sub(value);

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, value <= cpu.registers.y);

        Ok(0)
    }

    fn dec_decrement_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_sub(1);

        cpu.update_flags_zero_negative(result);
        cpu.overwrite(operand, result)?;

        Ok(0)
    }

    fn dex_decrement_index_x_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.x = cpu.registers.x.wrapping_sub(1);

        cpu.registers.set_status(StatusFlag::Zero,  cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative,  cpu.registers.x & 0x80 != 0);

        Ok(0)
    }

    fn dey_decrement_index_y_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.y = cpu.registers.y.wrapping_sub(1);

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(0)
    }

    fn eor_exclusive_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = cpu.registers.a ^ value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn inc_increment_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_add(1);

        cpu.update_flags_zero_negative(result);
        cpu.overwrite(operand, result)?;

        Ok(0)
    }

    fn inx_increment_index_x_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.x = cpu.registers.x.wrapping_add(1);

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(0)
    }

    fn iny_increment_index_y_by_one(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.y = cpu.registers.y.wrapping_add(1);

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(0)
    }

    fn jmp_jump_to_new_location(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;

        //debug!("CPU: preparing to jump to absolute address {:04X}", addr);
        cpu.registers.set_pc(addr);

        Ok(0)
    }

    fn jsr_jump_to_new_location_saving_return_address(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let pc = cpu.registers.safe_pc_add(2)?;

        cpu.push_stack((pc >> 8) as u8)?;
        cpu.push_stack(pc as u8)?;

        cpu.registers.set_pc(addr);

        Ok(0)
    }


    fn lda_load_accumulator_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn ldx_load_index_x_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.x = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(cycles)
    }

    fn ldy_load_index_y_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.y = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(cycles)
    }

    fn lsr_shift_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result = cpu.shift_right_and_update_carry_flags(value);

        cpu.update_flags_zero_negative(result);
        cpu.overwrite(operand, result)?;

        Ok(0)
    }

    fn nop_no_operation(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let cycle = cpu.get_cycles_by_page_crossing_for_load(operand);
        Ok(cycle)
    }

    fn ora_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = cpu.registers.a | value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn pha_push_accumulator_on_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let value = cpu.registers.a;

        cpu.push_stack(value)?;

        Ok(0)
    }

    /***
     * MN-23 status flags is wrong
     *
     *   https://www.masswerk.at/6502/6502_instruction_set.html#PHP
     *
     *   SR: N V - B D I Z C
     *       0 0 - - 0 0 1 1
     *
     *     PHP  ->  0 0 1 1 0 0 1 1  =  $33
     *     PLP  <-  0 0 - - 0 0 1 1  =  $03
     ***/
    fn php_push_processor_status_on_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let value = cpu.registers.p | StatusFlag::BreakCommand.bits() | StatusFlag::Unused.bits();
        cpu.push_stack(value)?;

        Ok(0)
    }

    fn pla_pull_accumulator_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let value = cpu.pop_stack()?;

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, value == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

        Ok(0)
    }

    /***
     * MN-23 status flags is wrong:
     *
     *   https://www.masswerk.at/6502/6502_instruction_set.html#PLP
     *
     *   SR: N V - B D I Z C
     *       0 0 - - 0 0 1 1
     *
     *     PHP  ->  0 0 1 1 0 0 1 1  =  $33
     *     PLP  <-  0 0 - - 0 0 1 1  =  $03
     ***/
    fn plp_pull_processor_status_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let status = cpu.pop_stack()?;

        cpu.registers.p = (cpu.registers.p & 0x30) | status & !0x30;

        Ok(0)
    }

    fn rol_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x01 } else { 0 };
        let result = (value << 1) | carry_in;

        cpu.overwrite(operand, result)?;

        cpu.registers.set_status(StatusFlag::Carry, value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(0)
    }

    fn ror_rotate_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
        let result = (value >> 1) | carry_in;

        cpu.overwrite(operand, result)?;

        cpu.registers.set_status(StatusFlag::Carry, value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(0)
    }

    fn rti_return_from_interrupt(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let status = cpu.pop_stack()?;

        cpu.registers.p = (status & 0xCF) | 0x20;

        let pcl = cpu.pop_stack()?;
        let pch = cpu.pop_stack()?;

        let addr = (pch as u16) << 8 | pcl as u16;
        cpu.registers.set_pc(addr);

        Ok(0)
    }

    fn rts_return_from_subroutine(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let pcl = cpu.pop_stack()?;
        let pch = cpu.pop_stack()?;

        let mut addr = (pch as u16) << 8 | pcl as u16;
        cpu.registers.set_pc(addr);

        addr = cpu.registers.safe_pc_add(1)?;
        cpu.registers.set_pc(addr);

        Ok(0)
    }

    fn sbc_subtract_memory_from_accumulator_with_borrow(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let carry = cpu.registers.get_status(StatusFlag::Carry) as u8;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        let a = cpu.registers.a;
        let (t0, overflow0) = a.overflowing_sub(value);
        let (t1, overflow1) = t0.overflowing_sub(1 - carry);

        cpu.registers.a = t1;

        cpu.registers.set_status(StatusFlag::Carry, !(overflow0 | overflow1));
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        let overflow = ((a ^ cpu.registers.a) & 0x80 != 0) && ((a ^ value) & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Overflow, overflow);

        Ok(cycles)
    }

    fn sec_set_carry_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::Carry, true);
        Ok(0)
    }

    fn sed_set_decimal_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::DecimalMode, true);
        Ok(0)
    }

    fn sei_set_interrupt_disable_status(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::InterruptDisable, true);
        Ok(0)
    }

    fn sta_store_accumulator_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;

        cpu.bus.borrow_mut().write_byte(addr, cpu.registers.a)?;

        Ok(0)
    }

    fn stx_store_index_x_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;

        cpu.bus.borrow_mut().write_byte(addr, cpu.registers.x)?;

        Ok(0)
    }

    fn sty_store_index_y_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;

        cpu.bus.borrow_mut().write_byte(addr, cpu.registers.y)?;

        Ok(0)
    }

    fn tax_transfer_accumulator_to_index_x(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.x = cpu.registers.a;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(0)
    }

    fn tay_transfer_accumulator_to_index_y(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.y = cpu.registers.a;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(0)
    }

    fn tsx_transfer_stack_pointer_to_index_x(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.x = cpu.registers.sp;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(0)
    }

    fn txa_transfer_index_x_to_accumulator(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.a = cpu.registers.x;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(0)
    }

    fn txs_transfer_index_x_to_stack_register(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.sp = cpu.registers.x;
        Ok(0)
    }

    fn tya_transfer_index_y_to_accumulator(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.a = cpu.registers.y;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(0)
    }

    fn alr_and_oper_plus_lsr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.and_and_memory_with_accumulator(cpu, operand)?;

        cpu.registers.set_status(StatusFlag::Carry, cpu.registers.a & 0x01 == 0x01);
        cpu.registers.a >>= 1;

        cpu.update_flags_zero_negative(cpu.registers.a);

        Ok(0)
    }

    fn anc_and_oper_plus_set_c_as_asl(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.and_and_memory_with_accumulator(cpu, operand)?;
        cpu.registers.set_status(StatusFlag::Carry, (cpu.registers.a >> 7) != 0);

        Ok(0)
    }

    fn anc_and_oper_plus_set_c_as_rol(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.anc_and_oper_plus_set_c_as_asl(cpu, operand)?;

        Ok(0)
    }

    fn ane_or_x_plus_and_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result = 0xFF & cpu.registers.x & value;

        cpu.registers.a = result;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(0)
    }

    fn arr_and_oper_plus_ror(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.and_and_memory_with_accumulator(cpu, operand)?;

        cpu.registers.set_status(StatusFlag::Overflow, (cpu.registers.a ^ (cpu.registers.a >> 1)) & 0x40 == 0x40);

        let c = cpu.registers.a >> 7;

        cpu.registers.a >>= 1 ;
        cpu.registers.a |= (cpu.registers.get_status(StatusFlag::Carry) as u8) << 7;

        cpu.registers.set_status(StatusFlag::Carry, c & 0x01 == 0x01);
        cpu.update_flags_zero_negative(cpu.registers.a);

        Ok(0)
    }

    fn dcp_dec_plus_cmp(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn dcp_dec_oper_plus_cmp_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.dec_decrement_memory_by_one(cpu, operand)?;
        self.cmp_compare_memory_with_accumulator(cpu, operand)?;
        Ok(0)
    }

    fn isb_inc_plus_sbc(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn isc_inc_oper_plus_sbc_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.inc_increment_memory_by_one(cpu, operand)?;
        self.sbc_subtract_memory_from_accumulator_with_borrow(cpu, operand)?;
        Ok(0)
    }

    fn jam_freeze_the_cpu(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Halted(cpu.registers.pc))
    }

    fn las_lda_tsx_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.lda_load_accumulator_with_memory(cpu, operand)?;
        self.tsx_transfer_stack_pointer_to_index_x(cpu, operand)?;

        Ok(0)
    }

    fn lax_lda_oper_plus_ldx_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        self.lda_load_accumulator_with_memory(cpu, operand)?;
        self.ldx_load_index_x_with_memory(cpu, operand)?;

        Ok(cycles)
    }

    fn lxa_store_and_oper_in_a_and_x(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.lax_lda_oper_plus_ldx_oper(cpu, operand)?;

        Ok(0)
    }

    fn rla_rol_oper_plus_and_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.rol_rotate_one_bit_left(cpu, operand)?;
        self.and_and_memory_with_accumulator(cpu, operand)?;

        Ok(0)
    }

    fn rra_ror_oper_plus_adc_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.ror_rotate_one_bit_right(cpu, operand)?;
        self.adc_add_memory_to_accumulator_with_carry(cpu, operand)?;

        Ok(0)
    }

    fn sax_axs_aax(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let result = cpu.registers.a & cpu.registers.x;

        cpu.bus.borrow_mut().write_byte(addr, result)?;

        Ok(0)
    }

    fn sbx_cmp_and_dex_at_once_sets_flags_like_cmp(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let result0 = cpu.registers.a & cpu.registers.x;
        let result1 = result0.wrapping_sub(value);

        cpu.update_flags_zero_negative(result1);
        cpu.registers.set_status(StatusFlag::Carry, result1 <= result0);

        cpu.registers.x = result1;

        Ok(0)
    }

    /***
     * holy shit
     * https://github.com/100thCoin/TriCNES/blob/main/Emulator.cs#L6379
     * missing:
     *  - Sometimes the actual value is stored in memory and the AND with <addrhi+1> part drops
     *   off (ex. SHY becomes true STY). This happens when the RDY line is used to stop the CPU
     *   (pulled low), i.e. either a 'bad line' or sprite DMA starts, in the second half of the cycle
     *   following the opcode fetch. 'For example, it never seems to occur if either the screen is
     *   blanked or C128 2MHz mode is enabled.' For this reason you will have to choose a
     *   suitable target address based on what kind of values you want to store. 'For $fe00
     *   there's no problem, since anding with $ff is the same as not anding. And if your values don't
     *   mind whether they are anded, e.g. if they are all $00-$7f for shy $7e00,x, there is also no
     *   difference whether the and works or not.'
     *  - https://hitmen.c02.at/files/docs/c64/NoMoreSecrets-NMOS6510UnintendedOpcodes-20162412.pdf
     ***/
    fn sha_stores_a_and_x_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {

        let (addr, page_crossed) = match (self.addressing_mode, operand) {
            (_, Operand::AddressAndEffectiveAddress(_, effective_addr, page_crossed)) => (effective_addr, *page_crossed),
            (_, Operand::Address(addr)) => (addr, false),
            _ => { unreachable!() }
        };
        let hi = (*addr >> 8) as u8;

        if page_crossed == true {
            let hi_unstable = hi & cpu.registers.x;
            let target = (*addr & 0x00FF) | ((hi_unstable as u16) << 8);
            let value = cpu.registers.a & (cpu.registers.x | 0xF5) & hi;
            cpu.bus.borrow_mut().write_byte(target, value)?;
        } else {
            let value = cpu.registers.a & cpu.registers.x & hi.wrapping_add(1);
            cpu.bus.borrow_mut().write_byte(*addr, value)?;
        };

        Ok(0)
    }

    fn shx_stores_x_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let strange_h1 = ((addr >> 8) as u8).wrapping_add(1);
        let result = cpu.registers.x & strange_h1;

        cpu.bus.borrow_mut().write_byte(addr, result)?;
        Ok(0)
    }

    fn shy_stores_y_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let strange_h1 = ((addr >> 8) as u8).wrapping_add(1);
        let result = cpu.registers.y & strange_h1;

        cpu.bus.borrow_mut().write_byte(addr, result)?;
        Ok(0)
    }

    fn slo_asl_oper_plus_ora_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.asl_shift_left_one_bit(cpu, operand)?;
        self.ora_or_memory_with_accumulator(cpu, operand)?;
        Ok(0)
    }

    fn sre_lsr_oper_plus_eor_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.lsr_shift_one_bit_right(cpu, operand)?;
        self.eor_exclusive_or_memory_with_accumulator(cpu, operand)?;
        Ok(0)
    }

    fn tax_puts_a_and_x_in_sp_and_stores_a_and_x_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.sha_stores_a_and_x_and_at_addr(cpu, operand)?;
        cpu.registers.sp = cpu.registers.a & cpu.registers.x;

        Ok(0)
    }

    fn usbc_sbc_oper_plus_nop(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.sbc_subtract_memory_from_accumulator_with_borrow(cpu, operand)?;
        self.nop_no_operation(cpu, operand)?;
        Ok(0)
    }

    fn illegal(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        warn!("CPU: illegal instruction at {:04X}", cpu.registers.pc);
        Ok(0)
    }
}