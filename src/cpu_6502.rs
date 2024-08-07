use std::thread::sleep;
use std::time::Duration;
use log::{info, LevelFilter};

use crate::cpu::{CPU, CpuError};
use crate::memory::Memory;

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
        self.registers.p = 0 | StatusFlag::InterruptDisable.bits();
        self.registers.sp = 0xFD;
        self.registers.pc = self.memory.read_word(0xFFFC)?;
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
        info!("- P (status): 0x{:08b}", self.registers.p);
        info!("- A (accumulator): 0x{:02x}", self.registers.a);
        info!("- X (index): 0x{:02x}", self.registers.x);
        info!("- Y (index): 0x{:02x}", self.registers.y);
        info!("- SP (stack pointer): 0x{:02x}", self.registers.sp);
        info!("- PC (program counter): 0x{:04x}", self.registers.pc);
    }

    fn run(&mut self) -> Result<(), CpuError> {
        info!("running CPU ...");

        loop {
            let opcode = self.memory.read_byte(self.registers.pc)?;
            self.execute_instruction(opcode)?;
            self.registers.pc += 1;

            sleep(Duration::from_secs(1));
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

    fn execute_instruction(&mut self, opcode: u8) -> Result<(), CpuError> {

    }
}