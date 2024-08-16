use std::collections::HashMap;
use std::{fmt, io};
use std::cell::RefCell;
use std::fmt::Display;
use std::fs::File;
use std::io::Write;
use std::process::exit;
use std::rc::Rc;
use std::thread::sleep;
use std::time::Duration;
use lazy_static::lazy_static;
use log::{debug, error, info};
use crate::bus::Bus;
use crate::cpu::{CPU, CpuError};
use crate::memory::{MemoryError};

//const CLOCK_HZ: usize = 1_789_773;
const STACK_BASE_ADDRESS: u16 = 0x0100;
const STACK_END_ADDRESS: u16 = 0x01FF;
const CYCLE_START_SEQUENCE: u32 = 7;

lazy_static! {
    static ref INSTRUCTIONS_TABLE: HashMap<u8, Instruction> = {
        let mut map = HashMap::<u8, Instruction>::new();

        macro_rules! add_instruction {
            ($map:ident, $opcode:expr, $op:ident, $addr_mode:ident, $bytes:expr, $cycles:expr, $exec:ident, $category:ident) => {
                $map.insert($opcode, Instruction {
                    opcode: OpCode::$op,
                    addressing_mode: AddressingMode::$addr_mode,
                    bytes: $bytes,
                    cycles: $cycles,
                    execute: Instruction::$exec,
                    category: InstructionCategory::$category
                })
            };
        }

        include!("instructions_macro_all.rs");
        map
    };
}

#[derive(Debug)]
enum OpCode {
    ADC, ALR, ANC, AND, ANE, ARR, ASL, BCC, BCS, BEQ, BIT, BMI, BNE, BPL, BRK, BVC, BVS, CLC, CLD,
    CLI, CLV, CMP, CPX, CPY, DCP, DEC, DEX, DEY, EOR, INC, INX, INY, ISB, ISC, JAM, JMP, JSR, LAS, 
    LAX, LDA, LDX, LDY, LSR, LXA, NOP, ORA, PHA, PHP, PLA, PLP, RLA, ROL, ROR, RRA, RTI, RTS, SAX, 
    SBC, SBX, SEC, SED, SEI, SHA, SHX, SHY, SLO, SRE, STA, STX, STY, TAX, TAY, TSX, TXA, TXS, TYA, 
    USBC
}

#[derive(Debug)]
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

#[derive(Debug, PartialEq)]
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
            .ok_or(MemoryError::OutOfRange(self.pc))?;

        Ok(pc)
    }
}

pub struct Cpu6502 {
    registers: Registers,
    bus: Rc<RefCell<dyn Bus>>,
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
        //cpu.memory.initialize()?;
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
        self.bus.borrow().dump();
    }

    fn run(&mut self) -> Result<(), CpuError> {
        info!("running CPU ...");

        let mut cycles = CYCLE_START_SEQUENCE;

        loop {
            debug!("pc: 0x{:04X}", self.registers.pc);
            let original_pc = self.registers.pc;

            let byte = self.bus.borrow().read_byte(self.registers.pc)?;
            let instruction = Cpu6502::decode_instruction(byte)?;
            let operand = self.fetch_operand(instruction)?;

            self.tracer.trace(&self, &instruction, &operand, cycles)?;

            let additional_cycles = self.execute_instruction(instruction, &operand)?;
            cycles = cycles + instruction.cycles + additional_cycles;

            /***if self.registers.pc == 0xDF63 {
                exit(1)
            }***/

            if original_pc == self.registers.pc {
                self.registers.pc = self.registers.safe_pc_add(instruction.bytes as i16)?;
            }

            self.instructions_executed += 1;
            sleep(Duration::from_millis(1));
        }
    }

    fn run_start_at(&mut self, address: u16) -> Result<(), CpuError> {
        self.registers.pc = address;

        debug!("pc set to address 0x{:04X} ...", address);
        self.run()
    }
}

impl Cpu6502 {
    pub fn new(bus: Rc<RefCell<dyn Bus>>, trace_file: Option<File>) -> Self {
        Cpu6502 {
            registers: Registers {
                a: 0,
                x: 0,
                y: 0,
                p: 0,
                sp: 0,
                pc: 0
            },
            bus,
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
        self.bus.borrow_mut().write_byte(addr, value)?;

        addr = addr - 1;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        debug!("sp (after push): 0x{:02X}", self.registers.sp);

        Ok(())
    }

    fn pop_stack(&mut self) -> Result<u8, CpuError> {
        let addr = STACK_BASE_ADDRESS + self.registers.sp as u16 + 1;

        debug!("sp (before pop): 0x{:02X}, popping at 0x{:02X}", self.registers.sp, addr);
        let value = self.bus.borrow().read_byte(addr)?;
        self.is_valid_stack_addr(addr)?;

        self.registers.sp = addr as u8;
        debug!("sp (after pop): 0x{:02X}, popped value {:02X}", self.registers.sp, value);

        Ok(value)
    }

    fn read_word_with_page_wrap(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.bus.borrow().read_byte(addr)?;

        let hi = if (addr & 0xFF) == 0xFF {
            self.bus.borrow().read_byte(addr & 0xFF00)?
        } else {
            self.bus.borrow().read_byte(addr.wrapping_add(1))?
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

    fn is_page_crossed(&self, addr1: u16, addr2: u16) -> bool {
        let page1 = addr1 & 0xFF00;
        let page2 = addr2 & 0xFF00;

        page1 != page2
    }

    fn get_cycles_by_page_crossing_for_conditional_jump(&self, source: u16, destination: u16) -> u32 {
        if self.is_page_crossed(source, destination) { 2 } else { 1 }
    }

    fn get_cycles_by_page_crossing_for_load(&self, operand: &Operand) -> u32 {
        match operand {
            Operand::AddressAndEffectiveAddress(_, _, page_crossed) => {
                debug!("||||||||||| {}", page_crossed);
                if *page_crossed { 1 } else { 0 }
            },
            _ => 0
        }
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

                Operand::Byte(self.bus.borrow().read_byte(pc)?)
            },

            AddressingMode::Absolute => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_word(pc)?;

                Operand::Address(addr)
            },

            AddressingMode::AbsoluteIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_word(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.x as u16);
                let page_crossed = self.is_page_crossed(addr, effective_addr);

                Operand::AddressAndEffectiveAddress(addr, effective_addr, page_crossed)
            }

            AddressingMode::AbsoluteIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_word(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.y as u16);
                let page_crossed = self.is_page_crossed(addr, effective_addr);

                debug!("============> addr: 0x{:04X}, effective_addr: 0x{:04X}, page_crossed: {}", addr, effective_addr, page_crossed);

                Operand::AddressAndEffectiveAddress(addr, effective_addr, page_crossed)
            }

            AddressingMode::ZeroPage => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_byte(pc)?;

                Operand::Address(addr as u16)
            },

            AddressingMode::ZeroPageIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_byte(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.x) as u16;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, false)
            },

            AddressingMode::ZeroPageIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_byte(pc)?;
                let effective_addr = addr.wrapping_add(self.registers.y) as u16;
                let page_crossed = self.is_page_crossed(addr as u16, effective_addr);

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, page_crossed)
            },

            AddressingMode::Indirect => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_word(pc)?;
                let effective_addr = self.read_word_with_page_wrap(addr)?;

                Operand::AddressAndEffectiveAddress(addr, effective_addr, false)
            },

            AddressingMode::IndirectIndexedX => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_byte(pc)?;
                let indirect_addr = addr.wrapping_add(self.registers.x);
                let effective_addr = self.read_word_with_page_wrap(indirect_addr as u16)?;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, false)
            },

            AddressingMode::IndirectIndexedY => {
                let pc = self.registers.safe_pc_add(1)?;
                let addr = self.bus.borrow().read_byte(pc)?;
                let indirect_addr = self.read_word_with_page_wrap(addr as u16)?;
                let effective_addr = indirect_addr.wrapping_add(self.registers.y as u16);
                let page_crossed = self.is_page_crossed(indirect_addr, effective_addr);

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, page_crossed)
            },

            AddressingMode::Relative => {
                let pc = self.registers.safe_pc_add(1)?;
                let offset = self.bus.borrow().read_byte(pc)? as i8;
                let addr = self.registers.safe_pc_add(2)?;
                let addr= addr.wrapping_add_signed(offset as i16);

                Operand::Address(addr)
            },
        };

        debug!("fetched operand: {}", operand);
        Ok(operand)
    }

    fn execute_instruction(&mut self, instruction: &Instruction, operand: &Operand) -> Result<u32, CpuError> {
        
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

    fn trace(&self, cpu: &Cpu6502, instruction: &Instruction, operand: &Operand, cycle: u32) -> Result<(), CpuError> {
        let a = format!("{:04X}", cpu.registers.pc);

        let mut b = format!("{:02X}", cpu.bus.borrow().read_byte(cpu.registers.pc)?);
        for i in 1..instruction.bytes {
            let o = format!(" {:02X}", cpu.bus.borrow().read_byte(cpu.registers.pc + i as u16)?);
            b.push_str(&o);
        }

        let c0 = if instruction.category == InstructionCategory::Illegal { "*" } else { " " };
        let c1 = format!("{:?}", &instruction.opcode);

        let c2 = match (&instruction.addressing_mode, operand, &instruction.opcode) {
            (AddressingMode::Implicit, _, _) => { "".to_string() },

            (AddressingMode::Accumulator, _, _) =>
                "A".to_string(),

            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JMP) |
            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JSR) =>
                format!("${:04X}", *addr),

            (AddressingMode::Absolute, Operand::Address(addr), _) =>
                format!("${:04X} = {:02X}", *addr, cpu.bus.borrow().read_byte(*addr)?),

            (AddressingMode::Relative, Operand::Address(addr), _) =>
                format!("${:04X}", *addr),

            (AddressingMode::ZeroPage, Operand::Address(addr), _) =>
                format!("${:02X} = {:02X}", *addr as u8, cpu.bus.borrow().read_byte(*addr)?),

            (AddressingMode::AbsoluteIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},X @ {:04X} = {:02X}", *addr, *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::AbsoluteIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},Y @ {:04X} = {:02X}", *addr, *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},X @ {:02X} = {:02X}", *addr, *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},Y @ {:02X} = {:02X}", *addr, *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::Indirect, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:04X}) = {:04X}", *addr, effective),

            (AddressingMode::IndirectIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X},X) @ {:02X} = {:04X} = {:02X}", *addr, (*addr as u8).wrapping_add(cpu.registers.x), *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::IndirectIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X}),Y = {:04X} @ {:04X} = {:02X}", *addr, effective.wrapping_sub(cpu.registers.y as u16), *effective, cpu.bus.borrow().read_byte(*effective)?),

            (AddressingMode::Immediate, Operand::Byte(byte), _) =>
                format!("#${:02X}", byte),

            _ => {
                return Err(CpuError::InvalidOperand(
                    format!("could not format instruction and operand: {:?}, {:?}",
                            &instruction.addressing_mode, operand)
                ))
            }
        };
        let c = format!("{}{} {}", c0, c1, c2);

        let d = format!("A:{:02X} X:{:02X} Y:{:02X} P:{:02X} SP:{:02X}",
                        cpu.registers.a, cpu.registers.x, cpu.registers.y, cpu.registers.p, cpu.registers.sp);

        let e = format!("PPU:{:>3},{:>3}", 0, 0);

        let f = format!("CYC:{}", cycle);

        let mut output = self.trace.borrow_mut();

        write!(output, "{:<6}{:<9}", a, b)?;
        write!(output, "{:<padding$}", c, padding = 33)?;
        write!(output, "{:<26}", d)?;
        write!(output, "{:<12}", e)?;
        write!(output, "{}", f)?;

        writeln!(output)?;

        Ok(())
    }
}

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
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 1 } else { 0 };
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        let result = cpu.registers.a as u16 + value as u16 + carry_in as u16;

        cpu.registers.set_status(StatusFlag::Carry, result > 0xFF);
        cpu.registers.set_status(StatusFlag::Zero, result & 0xFF == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        let overflow = !(cpu.registers.a ^ value) & (cpu.registers.a ^ result as u8) & 0x80;
        cpu.registers.set_status(StatusFlag::Overflow, overflow != 0);

        cpu.registers.a = result as u8;

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

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bcs_branch_on_carry_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Carry) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn beq_branch_on_result_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Zero) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

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

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bne_branch_on_result_not_zero(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Zero) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bpl_branch_on_result_plus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Negative) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

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
        cpu.registers.pc = cpu.bus.borrow().read_word(0xFFFE)?;

        Ok(0)
    }

    fn bvc_branch_on_overflow_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Overflow) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bvs_branch_on_overflow_set(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Overflow) {
            let addr = cpu.get_operand_word_value(operand)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(cpu.registers.safe_pc_add(self.bytes as i16)?, addr);

            debug!("branching to address {:04X}", addr);
            cpu.registers.pc = addr;

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

        debug!("preparing to jump to absolute address {:04X}", addr);
        cpu.registers.pc = addr;

        Ok(0)
    }

    fn jsr_jump_to_new_location_saving_return_address(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let pc = cpu.registers.safe_pc_add(2)?;

        cpu.push_stack((pc >> 8) as u8)?;
        cpu.push_stack(pc as u8)?;

        cpu.registers.pc = addr;

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

    fn php_push_processor_status_on_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let status = cpu.registers.p | 0x30;

        cpu.push_stack(status)?;

        Ok(0)
    }

    fn pla_pull_accumulator_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let value = cpu.pop_stack()?;

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, value == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

        Ok(0)
    }

    fn plp_pull_processor_status_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let status = cpu.pop_stack()?;

        cpu.registers.p = (status & 0xCF) | 0x20;

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

        cpu.registers.pc = (pch as u16) << 8 | pcl as u16;

        Ok(0)
    }

    fn rts_return_from_subroutine(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let pcl = cpu.pop_stack()?;
        let pch = cpu.pop_stack()?;

        cpu.registers.pc = (pch as u16) << 8 | pcl as u16;
        cpu.registers.pc = cpu.registers.safe_pc_add(1)?;

        Ok(0)
    }

    fn sbc_subtract_memory_from_accumulator_with_borrow(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let value = cpu.get_operand_byte_value(operand)?;
        let borrow = !cpu.registers.get_status(StatusFlag::Carry) as u8;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        let temp = value.wrapping_add(borrow);
        let result = cpu.registers.a.wrapping_sub(temp);

        cpu.registers.set_status(StatusFlag::Carry, cpu.registers.a >= temp);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80!= 0);

        let overflow = ((cpu.registers.a ^ result) & 0x80 != 0) && ((cpu.registers.a ^ value) & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Overflow, overflow);
        cpu.registers.a = result;

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

    fn alr_and_oper_plus_lsr(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn anc_and_oper_plus_set_c_as_asl(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn anc_and_oper_plus_set_c_as_rol(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn ane_or_x_plus_and_oper(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn arr_and_oper_plus_ror(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn dcp__dec_plus_cmp(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
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

    fn las_lda_tsx_oper(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn lax_lda_oper_plus_ldx_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        self.lda_load_accumulator_with_memory(cpu, operand)?;
        self.ldx_load_index_x_with_memory(cpu, operand)?;

        Ok(cycles)
    }

    fn lxa_store_and_oper_in_a_and_x(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
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

    fn sax_axs__aax(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        let addr = cpu.get_operand_word_value(operand)?;
        let result = cpu.registers.a & cpu.registers.x;

        cpu.bus.borrow_mut().write_byte(addr, result)?;
        Ok(0)
    }

    fn sbx_cmp_and_dex_at_once__sets_flags_like_cmp(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn sha_stores_a_and_x_and_at_addr(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn shx_stores_x_and_at_addr(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn shy_stores_y_and_at_addr(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
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

    fn tax_puts_a_and_x_in_sp_and_stores_a_and_x_and_at_addr(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn usbc_sbc_oper_plus_nop(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        self.sbc_subtract_memory_from_accumulator_with_borrow(cpu, operand)?;
        self.nop_no_operation(cpu, operand)?;
        Ok(0)
    }
}