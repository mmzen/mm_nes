// Authorship: Human 18% | Claude 82%
use std::fmt;
use std::cell::RefCell;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use log::{error, info, warn};
use once_cell::sync::Lazy;
use crate::bus::Bus;
use crate::cpu::{BusOperation, CPU, CpuBusIntent, CpuCycleResult, CpuError, Interruptible};
use crate::cpu_debugger::CpuSnapshot;
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

#[derive(Debug, Clone, Copy, PartialEq)]
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
    /// Address with pre-read value - used by cycle-accurate mode to avoid double reads
    AddressWithValue(u16, u8),
    AddressAndEffectiveAddress(u16, u16, bool),
    /// Address/effective address with pre-read value - used by cycle-accurate mode
    AddressEffectiveWithValue(u16, u16, bool, u8),
    Accumulator,
    None
}

impl Display for Operand {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Operand::Byte(val) => write!(f, "byte: 0x{:02X}", val),
            Operand::Address(addr) => write!(f, "word: 0x{:04X}", addr),
            Operand::AddressWithValue(addr, val) => write!(f, "word: 0x{:04X} = 0x{:02X}", addr, val),
            Operand::Accumulator => write!(f, "accumulator"),
            Operand::None => { write!(f, "none") }
            Operand::AddressAndEffectiveAddress(addr, effective, page_crossed) => {
                write!(f, "address: 0x{:04X}, effective address: 0x{:04X}, page crossed: {}", addr, effective, page_crossed)
            }
            Operand::AddressEffectiveWithValue(addr, effective, page_crossed, val) => {
                write!(f, "address: 0x{:04X}, effective: 0x{:04X}, page crossed: {}, value: 0x{:02X}", addr, effective, page_crossed, val)
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
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
        let byte0 = bus.borrow().peek_byte(registers.pc)?;
        bytes.push(byte0);

        for i in 1..instruction.bytes {
            let byte = bus.borrow().peek_byte(registers.safe_pc_add(i as i16)?)?;
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

            (AddressingMode::Absolute, Operand::Address(addr), OpCode::JMP) =>
                format!("${:04X}", *addr),

            // JSR handles operand fetching internally, so we read address for display
            (AddressingMode::Absolute, Operand::None, OpCode::JSR) => {
                let pc_plus_1 = registers.safe_pc_add(1)?;
                let addr = bus.borrow().peek_byte(pc_plus_1)? as u16
                    | ((bus.borrow().peek_byte(registers.safe_pc_add(2)?)? as u16) << 8);
                format!("${:04X}", addr)
            },

            (AddressingMode::Absolute, Operand::Address(addr), _) =>
                format!("${:04X} = {:02X}", *addr, bus.borrow().peek_byte(*addr)?),

            (AddressingMode::Relative, Operand::Address(addr), _) =>
                format!("${:04X}", *addr),

            (AddressingMode::ZeroPage, Operand::Address(addr), _) =>
                format!("${:02X} = {:02X}", *addr as u8, bus.borrow().peek_byte(*addr)?),

            (AddressingMode::AbsoluteIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},X @ {:04X} = {:02X}", *addr, *effective, bus.borrow().peek_byte(*effective)?),

            (AddressingMode::AbsoluteIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:04X},Y @ {:04X} = {:02X}", *addr, *effective, bus.borrow().peek_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},X @ {:02X} = {:02X}", *addr, *effective, bus.borrow().peek_byte(*effective)?),

            (AddressingMode::ZeroPageIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("${:02X},Y @ {:02X} = {:02X}", *addr, *effective, bus.borrow().peek_byte(*effective)?),

            (AddressingMode::Indirect, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:04X}) = {:04X}", *addr, *effective),

            (AddressingMode::IndirectIndexedX, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X},X) @ {:02X} = {:04X} = {:02X}", *addr, (*addr as u8).wrapping_add(registers.x),
                        *effective, bus.borrow().peek_byte(*effective)?),

            (AddressingMode::IndirectIndexedY, Operand::AddressAndEffectiveAddress(addr, effective, _), _) =>
                format!("(${:02X}),Y = {:04X} @ {:04X} = {:02X}", *addr, (*effective).wrapping_sub(registers.y as u16),
                        *effective, bus.borrow().peek_byte(*effective)?),

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

/// Intermediate state data during instruction execution.
/// Holds values fetched on previous cycles that are needed for later cycles.
#[derive(Debug, Clone, Default)]
struct InstructionState {
    /// First operand byte (low byte of address or immediate value)
    operand_lo: u8,
    /// Second operand byte (high byte of address)
    operand_hi: u8,
    /// Computed effective address (after indexing)
    effective_addr: u16,
    /// Base address (before indexing, for indirect modes)
    base_addr: u16,
    /// Whether page boundary was crossed (affects cycle count for some operations)
    page_crossed: bool,
    /// Value read from effective address (for RMW operations or ALU input)
    read_value: u8,
    /// Whether branch was taken (for relative addressing)
    branch_taken: bool,
    /// For branches: the target address after taking the branch
    branch_target: u16,
}

/// Type of hardware interrupt being serviced
#[derive(Debug, Clone, Copy, PartialEq)]
enum InterruptType {
    Nmi,
    Irq,
}

/// State of the CPU cycle-stepping state machine.
/// Tracks where we are in the execution of the current instruction.
#[derive(Debug, Clone)]
enum CpuCycleState {
    /// CPU is ready to fetch the next instruction opcode.
    FetchOpcode,
    /// CPU is executing an instruction cycle-by-cycle.
    Executing {
        /// The opcode byte (for decode)
        opcode: u8,
        /// The instruction being executed (from decode table)
        instruction: &'static Instruction,
        /// Current cycle within instruction (1-based, 1 = after opcode fetch)
        cycle: u8,
        /// Intermediate state accumulated during execution
        state: InstructionState,
        /// True if this is an interrupt sequence (NMI/IRQ), not a regular instruction
        is_interrupt: bool,
    },
    /// CPU is executing a hardware interrupt sequence (NMI or IRQ).
    /// This is separate from BRK which is a software interrupt instruction.
    /// The interrupt sequence takes 7 cycles:
    /// - Cycle 1-2: Internal operations / dummy reads
    /// - Cycle 3: Push PCH to stack
    /// - Cycle 4: Push PCL to stack
    /// - Cycle 5: Push P to stack (without B flag)
    /// - Cycle 6: Read interrupt vector low byte
    /// - Cycle 7: Read interrupt vector high byte, jump to handler
    InterruptSequence {
        /// Type of interrupt being serviced
        interrupt_type: InterruptType,
        /// Current cycle within interrupt sequence (1-7)
        cycle: u8,
        /// Base address of interrupt vector ($FFFA for NMI, $FFFE for IRQ)
        /// This is set when entering the sequence and updated if NMI hijacks IRQ
        vector_base: u16,
        /// Intermediate state for vector address
        state: InstructionState,
    },
    /// CPU is halted, waiting for DMA to complete.
    Halted {
        /// Cycles remaining in halt state
        cycles_remaining: u32,
    },
}

impl Default for CpuCycleState {
    fn default() -> Self {
        CpuCycleState::FetchOpcode
    }
}

#[derive(Debug)]
pub struct Cpu6502 {
    registers: Registers,
    bus: Rc<RefCell<dyn Bus>>,
    instructions_executed: u64,
    interrupt: InterruptMask,
    nmi_line_low: bool,
    pending_nmi: bool,
    pending_i_flag: Option<bool>,
    cycles: u32,
    /// Cycle-stepping state machine state
    cycle_state: CpuCycleState,
    /// NMI state latched on the penultimate cycle of the current instruction.
    /// This is what gets checked after instruction completion.
    latched_nmi: bool,
    /// IRQ state latched on the penultimate cycle of the current instruction.
    /// This is what gets checked after instruction completion.
    latched_irq: bool,
    /// Interrupt type to service at the next FetchOpcode boundary.
    /// Set when an instruction completes with an interrupt pending.
    /// This ensures interrupt cycle 1 always happens in FetchOpcode.
    pending_interrupt: Option<InterruptType>,
    /// Pending RMW result value for cycle-accurate mode.
    /// When set, rmw_overwrite skips bus writes and stores the result here instead.
    /// The cycle-accurate code then retrieves this value and does the write itself.
    rmw_pending_value: Option<u8>,
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

    /// Signal NMI to the CPU (falling edge detection).
    ///
    /// NMI is edge-triggered: this sets `pending_nmi` only on the transition from
    /// high to low (i.e., only if the line wasn't already low). Once triggered,
    /// `pending_nmi` remains true until the CPU services the interrupt.
    ///
    /// # NMI Line Clearing Contract
    ///
    /// **IMPORTANT**: The caller (typically PPU) is responsible for calling `clear_nmi()`
    /// when the NMI line is released (goes high). The CPU does NOT clear `nmi_line_low`
    /// when servicing NMI - it only clears `pending_nmi`. If the caller never calls
    /// `clear_nmi()`, subsequent `signal_nmi()` calls will be ignored (line stays low),
    /// effectively deadlocking NMI.
    ///
    /// Typical PPU integration:
    /// 1. PPU sets NMI at start of vblank: `cpu.signal_nmi()`
    /// 2. PPU clears NMI when vblank ends or when $2002 is read: `cpu.clear_nmi()`
    fn signal_nmi(&mut self) -> Result<(), CpuError> {
        if !self.nmi_line_low {
            self.nmi_line_low = true;
            self.pending_nmi = true;
        }
        Ok(())
    }

    /// Clear the NMI line (release from low to high).
    ///
    /// This must be called by the NMI source (typically PPU) when the condition
    /// that caused NMI is cleared. This allows future `signal_nmi()` calls to
    /// trigger new NMI edges.
    ///
    /// Note: This does NOT clear `pending_nmi`. If NMI was already triggered and
    /// hasn't been serviced yet, it will still be serviced. This only allows
    /// future edges to be detected.
    fn clear_nmi(&mut self) -> Result<(), CpuError> {
        self.nmi_line_low = false;
        Ok(())
    }

    fn is_asserted_nmi(&self) -> Result<bool, CpuError> {
        Ok(self.pending_nmi)
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
        self.instructions_executed = 0;
        self.cycles = 0;

        self.interrupt = InterruptMask::default();
        self.nmi_line_low = false;
        self.pending_nmi = false;
        self.pending_i_flag = None;
        self.pending_interrupt = None;
        self.cycle_state = CpuCycleState::FetchOpcode;
        self.latched_nmi = false;
        self.latched_irq = false;
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

    fn step_cycle(&mut self) -> Result<CpuCycleResult, CpuError> {
        // Handle halted state (DMA)
        if let CpuCycleState::Halted { cycles_remaining } = &mut self.cycle_state {
            *cycles_remaining -= 1;
            self.cycles += 1;

            if *cycles_remaining == 0 {
                self.cycle_state = CpuCycleState::FetchOpcode;
            }

            return Ok(CpuCycleResult {
                halted: true,
                cycle_description: "CPU halted (DMA)",
                ..Default::default()
            });
        }

        match &self.cycle_state {
            CpuCycleState::FetchOpcode => {
                // Determine if an interrupt should be serviced.
                // Priority: pending_interrupt (set by previous instruction) > poll/latch
                //
                // If pending_interrupt is set, we already decided to service an interrupt
                // at the end of the previous instruction. Don't re-poll.
                //
                // If pending_interrupt is None, poll and check latch as usual.
                let int_type = self.pending_interrupt.take().or_else(|| {
                    // Clear latched state and poll fresh
                    self.latched_nmi = false;
                    self.latched_irq = false;
                    self.poll_interrupts();
                    self.check_interrupt_from_latch()
                });

                // CRITICAL: Check for pending interrupt BEFORE fetching opcode
                // If an interrupt is pending at this instruction boundary, we must
                // NOT execute the next opcode. Instead, enter the interrupt sequence
                // immediately. The first cycle performs a dummy read at PC.
                if let Some(int_type) = int_type {
                    // Clear pending_nmi if we're servicing NMI
                    if int_type == InterruptType::Nmi {
                        self.pending_nmi = false;
                    }

                    // Clear latches now that we're committing to service
                    self.latched_nmi = false;
                    self.latched_irq = false;

                    // Perform dummy read at current PC (interrupt cycle 1)
                    let pc = self.registers.pc;
                    let data = self.bus.borrow().read_byte(pc)?;
                    self.cycles += 1;

                    // Compute initial vector address based on interrupt type
                    let mut vector_base = match int_type {
                        InterruptType::Nmi => NMI_VECTOR,
                        InterruptType::Irq => IRQ_VECTOR,
                    };

                    // NMI hijack check for cycle 1: If we committed to IRQ but NMI
                    // arrived during the dummy read (signaled externally), hijack now.
                    // This covers the full hijack window: cycle 1 here + cycles 2-4 in
                    // InterruptSequence. Hijack is allowed until push PCH (cycle 3).
                    // When hijack occurs, update BOTH vector_base AND int_type so the
                    // state machine consistently reflects that we're now servicing NMI.
                    let mut int_type = int_type;
                    if int_type == InterruptType::Irq && self.pending_nmi {
                        self.pending_nmi = false;
                        vector_base = NMI_VECTOR;
                        int_type = InterruptType::Nmi;
                    }

                    // Transition to interrupt sequence - cycle 2 is next
                    self.cycle_state = CpuCycleState::InterruptSequence {
                        interrupt_type: int_type,
                        cycle: 2,
                        vector_base,
                        state: InstructionState::default(),
                    };

                    return Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(pc),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Interrupt: dummy read (PC) [cycle 1]",
                        ..Default::default()
                    });
                }

                // No interrupt pending - fetch opcode as normal
                let pc = self.registers.pc;
                let opcode = self.bus.borrow().read_byte(pc)?;
                let instruction = Cpu6502::decode_instruction(opcode)?;

                // Move to executing state - cycle 1 means "after opcode fetch"
                self.cycle_state = CpuCycleState::Executing {
                    opcode,
                    instruction,
                    cycle: 1,
                    state: InstructionState::default(),
                    is_interrupt: false,
                };

                self.cycles += 1;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(pc),
                    data: Some(opcode),
                    bus_op: BusOperation::Read,
                    cycle_description: "Fetch opcode",
                    ..Default::default()
                })
            }

            CpuCycleState::Executing { opcode, instruction, cycle, state, is_interrupt } => {
                let opcode = *opcode;
                let instruction = *instruction;
                let cycle = *cycle;
                let mut state = state.clone();
                let is_interrupt = *is_interrupt;

                // Poll interrupts at the START of each cycle (before execution)
                // The latched state from the final cycle's poll will be used for interrupt handling
                self.poll_interrupts();

                self.cycles += 1;

                // Execute the appropriate action for this cycle based on addressing mode
                let result = self.execute_cycle(opcode, instruction, cycle, &mut state, is_interrupt)?;

                // Check if instruction is complete
                if result.instruction_complete {
                    // Check for interrupts using the LATCHED state from the final cycle's poll
                    // If an interrupt is pending, record it and go to FetchOpcode.
                    // FetchOpcode will then perform interrupt cycle 1 (dummy read at PC).
                    // This ensures there's ONE canonical place for interrupt cycle 1.
                    if let Some(int_type) = self.check_interrupt_from_latch() {
                        // Record which interrupt to service at the next FetchOpcode boundary
                        self.pending_interrupt = Some(int_type);
                        // Clear latches immediately to prevent stale state persisting
                        self.latched_nmi = false;
                        self.latched_irq = false;
                    }
                    // Always transition to FetchOpcode - it handles interrupt entry
                    self.cycle_state = CpuCycleState::FetchOpcode;
                } else {
                    // Continue to next cycle
                    self.cycle_state = CpuCycleState::Executing {
                        opcode,
                        instruction,
                        cycle: cycle + 1,
                        state,
                        is_interrupt,
                    };
                }

                Ok(result)
            }

            CpuCycleState::InterruptSequence { interrupt_type, cycle, vector_base, state } => {
                let mut interrupt_type = *interrupt_type;
                let cycle = *cycle;
                let mut vector_base = *vector_base;
                let mut state = state.clone();

                // Poll interrupts at the START of each cycle
                // NMI can hijack an IRQ sequence if it arrives during cycles 1-4
                self.poll_interrupts();

                // Check for NMI hijack during cycles 2-4 (cycle 1 is done in FetchOpcode)
                // If NMI arrives while processing IRQ, it hijacks the vector.
                // When hijack occurs, update BOTH vector_base AND interrupt_type so the
                // state machine consistently reflects that we're now servicing NMI.
                if interrupt_type == InterruptType::Irq && cycle <= 4 && self.pending_nmi {
                    self.pending_nmi = false;
                    vector_base = NMI_VECTOR;
                    interrupt_type = InterruptType::Nmi;
                }

                self.cycles += 1;

                // Execute one cycle of the interrupt sequence
                let result = self.execute_interrupt_cycle(cycle, vector_base, &mut state)?;

                if result.instruction_complete {
                    // Interrupt sequence complete, go to next instruction
                    self.cycle_state = CpuCycleState::FetchOpcode;
                } else {
                    // Continue to next cycle of interrupt sequence
                    self.cycle_state = CpuCycleState::InterruptSequence {
                        interrupt_type,
                        cycle: cycle + 1,
                        vector_base,
                        state,
                    };
                }

                Ok(result)
            }

            CpuCycleState::Halted { .. } => {
                // Already handled above
                unreachable!()
            }
        }
    }

    fn is_mid_instruction(&self) -> bool {
        matches!(self.cycle_state, CpuCycleState::Executing { .. } | CpuCycleState::InterruptSequence { .. })
    }

    fn get_instruction_cycle(&self) -> Option<u8> {
        match &self.cycle_state {
            CpuCycleState::FetchOpcode => Some(0),
            CpuCycleState::Executing { cycle, .. } => Some(*cycle as u8),
            CpuCycleState::InterruptSequence { cycle, .. } => Some(*cycle as u8),
            CpuCycleState::Halted { .. } => None,
        }
    }

    fn get_current_opcode(&self) -> Option<u8> {
        match &self.cycle_state {
            CpuCycleState::Executing { opcode, .. } => Some(*opcode),
            // During interrupt sequence, there's no opcode (hardware interrupt)
            CpuCycleState::InterruptSequence { .. } => None,
            _ => None,
        }
    }

    fn is_halted(&self) -> bool {
        matches!(self.cycle_state, CpuCycleState::Halted { .. })
    }

    fn halt_cycles(&mut self, cycles: u32) {
        if cycles > 0 {
            self.cycle_state = CpuCycleState::Halted { cycles_remaining: cycles };
        }
    }

    fn get_cycles(&self) -> u32 {
        self.cycles
    }

    fn get_pending_bus_operation(&self) -> CpuBusIntent {
        self.compute_pending_bus_intent()
    }
}

impl Cpu6502 {
    /// Compute the bus intent for the next cycle based on current state.
    /// This is used by DMA to know whether to halt the CPU (can only halt on read cycles).
    fn compute_pending_bus_intent(&self) -> CpuBusIntent {
        match &self.cycle_state {
            CpuCycleState::FetchOpcode => {
                // Will fetch opcode from PC
                CpuBusIntent {
                    op: BusOperation::Read,
                    address: Some(self.registers.pc),
                    is_write: false,
                }
            }

            CpuCycleState::Executing { opcode, instruction, cycle, state, .. } => {
                // Determine bus intent based on addressing mode and cycle
                self.compute_executing_bus_intent(*opcode, instruction, *cycle, state)
            }

            CpuCycleState::InterruptSequence { cycle, vector_base, .. } => {
                // Interrupt sequence cycles 2-7 (cycle 1 is done in FetchOpcode):
                // - Cycle 2: dummy read PC
                // - Cycles 3-5: push PCH, PCL, P to stack (SP decrements after each)
                // - Cycles 6-7: read interrupt vector
                match cycle {
                    2 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(self.registers.pc),
                        is_write: false,
                    },
                    3 | 4 | 5 => {
                        // Stack push cycles use current SP, then decrement after write.
                        // Intent must report the actual address that will be written:
                        // - Cycle 3: writes to SP (current), then SP--
                        // - Cycle 4: writes to SP (decremented once), then SP--
                        // - Cycle 5: writes to SP (decremented twice), then SP--
                        // Since execute_interrupt_cycle uses current SP at start of each
                        // cycle and decrements after, and state transitions happen after
                        // execution, the current SP should already reflect prior decrements.
                        CpuBusIntent {
                            op: BusOperation::Write,
                            address: Some(STACK_BASE_ADDRESS + self.registers.sp as u16),
                            is_write: true,
                        }
                    }
                    6 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(*vector_base),
                        is_write: false,
                    },
                    7 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(*vector_base + 1),
                        is_write: false,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            CpuCycleState::Halted { .. } => {
                // Halted - no bus operation (or repeated read, handled separately)
                CpuBusIntent::default()
            }
        }
    }

    /// Compute bus intent for an executing instruction.
    /// This is complex because it depends on addressing mode and cycle.
    fn compute_executing_bus_intent(
        &self,
        _opcode: u8,
        instruction: &'static Instruction,
        cycle: u8,
        state: &InstructionState,
    ) -> CpuBusIntent {
        use AddressingMode::*;

        let pc = self.registers.pc;

        // Check if this is a store instruction (STA, STX, STY, SAX, SHA, SHX, SHY)
        let is_store = matches!(
            instruction.opcode,
            OpCode::STA | OpCode::STX | OpCode::STY | OpCode::SAX |
            OpCode::SHA | OpCode::SHX | OpCode::SHY
        );

        // Check if this is an RMW instruction (ASL, LSR, ROL, ROR, INC, DEC, etc.)
        let is_rmw = matches!(
            instruction.opcode,
            OpCode::ASL | OpCode::LSR | OpCode::ROL | OpCode::ROR |
            OpCode::INC | OpCode::DEC | OpCode::DCP | OpCode::ISB |
            OpCode::SLO | OpCode::SRE | OpCode::RLA | OpCode::RRA
        ) && !matches!(instruction.addressing_mode, Accumulator | Implicit);

        match instruction.addressing_mode {
            Implicit | Accumulator => {
                // 2-cycle implicit: cycle 1 is dummy read
                CpuBusIntent {
                    op: BusOperation::Read,
                    address: Some(pc.wrapping_add(1)),
                    is_write: false,
                }
            }

            Immediate => {
                // Cycle 1: read operand
                CpuBusIntent {
                    op: BusOperation::Read,
                    address: Some(pc.wrapping_add(1)),
                    is_write: false,
                }
            }

            ZeroPage => {
                match cycle {
                    1 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(1)),
                        is_write: false,
                    },
                    2 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.operand_lo as u16),
                        is_write: true,
                    },
                    2 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(state.operand_lo as u16),
                        is_write: false,
                    },
                    3 if is_rmw => CpuBusIntent {
                        // RMW: write back original value
                        op: BusOperation::Write,
                        address: Some(state.operand_lo as u16),
                        is_write: true,
                    },
                    4 if is_rmw => CpuBusIntent {
                        // RMW: write modified value
                        op: BusOperation::Write,
                        address: Some(state.operand_lo as u16),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            ZeroPageIndexedX | ZeroPageIndexedY => {
                match cycle {
                    1 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(1)),
                        is_write: false,
                    },
                    2 => CpuBusIntent {
                        // Dummy read from unindexed address
                        op: BusOperation::Read,
                        address: Some(state.operand_lo as u16),
                        is_write: false,
                    },
                    3 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    3 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    4 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    5 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            Absolute => {
                match cycle {
                    1 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(1)),
                        is_write: false,
                    },
                    2 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(2)),
                        is_write: false,
                    },
                    3 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    3 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    4 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    5 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            AbsoluteIndexedX | AbsoluteIndexedY => {
                // Complex: may have page crossing penalty
                match cycle {
                    1 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(1)),
                        is_write: false,
                    },
                    2 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(pc.wrapping_add(2)),
                        is_write: false,
                    },
                    3 => CpuBusIntent {
                        // Dummy read from uncarried address or real read
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    4 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    4 if is_rmw => CpuBusIntent {
                        // Page cross fix read
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    4 => CpuBusIntent {
                        // Page cross fix read or completion
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    5 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    6 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            IndirectIndexedX => {
                // (ZP,X): complex indirect addressing
                match cycle {
                    1..=4 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: None, // Complex to compute
                        is_write: false,
                    },
                    5 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    5 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    6 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    7 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            IndirectIndexedY => {
                // (ZP),Y: complex indirect addressing
                match cycle {
                    1..=3 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: None, // Complex to compute
                        is_write: false,
                    },
                    4 if is_store => CpuBusIntent {
                        // Dummy read then store
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    4 => CpuBusIntent {
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    5 if is_store => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    5 => CpuBusIntent {
                        // Page cross fix or RMW read
                        op: BusOperation::Read,
                        address: Some(state.effective_addr),
                        is_write: false,
                    },
                    6 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    7 if is_rmw => CpuBusIntent {
                        op: BusOperation::Write,
                        address: Some(state.effective_addr),
                        is_write: true,
                    },
                    _ => CpuBusIntent::default(),
                }
            }

            Relative => {
                // Branch instructions
                CpuBusIntent {
                    op: BusOperation::Read,
                    address: Some(pc.wrapping_add(1)),
                    is_write: false,
                }
            }

            Indirect => {
                // JMP (indirect) - all reads
                CpuBusIntent {
                    op: BusOperation::Read,
                    address: None,
                    is_write: false,
                }
            }
        }
    }

    /// Execute a single cycle of an instruction.
    /// Returns the CpuCycleResult describing what happened this cycle.
    /// Updates `state` with any intermediate values computed.
    fn execute_cycle(
        &mut self,
        _opcode: u8,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        use AddressingMode::*;

        let pc = self.registers.pc;

        match instruction.addressing_mode {
            // ========== Implicit addressing mode ==========
            // Most implicit instructions are 2 cycles, but stack operations have special timing
            Implicit => {
                // Check for stack operations that have special timing
                match instruction.opcode {
                    // PHA/PHP: 3 cycles - fetch, dummy read, write to stack
                    OpCode::PHA | OpCode::PHP => {
                        self.execute_cycle_push_stack(instruction, cycle, state, is_interrupt)
                    }
                    // PLA/PLP: 4 cycles - fetch, dummy read, dummy stack read, read from stack
                    OpCode::PLA | OpCode::PLP => {
                        self.execute_cycle_pull_stack(instruction, cycle, state, is_interrupt)
                    }
                    // RTS: 6 cycles - special return timing
                    OpCode::RTS => {
                        self.execute_cycle_rts(instruction, cycle, state, is_interrupt)
                    }
                    // RTI: 6 cycles - special return timing
                    OpCode::RTI => {
                        self.execute_cycle_rti(instruction, cycle, state, is_interrupt)
                    }
                    // BRK: 7 cycles - interrupt sequence
                    OpCode::BRK => {
                        self.execute_cycle_brk(instruction, cycle, state, is_interrupt)
                    }
                    // JAM/KIL/HLT: 11 cycles - reads from interrupt vector area
                    OpCode::JAM => {
                        self.execute_cycle_jam(instruction, cycle, state, is_interrupt)
                    }
                    // All other implicit instructions: 2 cycles
                    _ => {
                        // cycle=1 means we're on the 2nd cycle (after opcode fetch)
                        let addr = pc.wrapping_add(1);
                        let data = self.bus.borrow().read_byte(addr)?; // dummy read

                        // Execute the instruction
                        if !is_interrupt {
                            let operand = Operand::None;
                            self.execute_instruction(&instruction, &operand)?;
                            self.finalize_instruction(instruction)?;
                        }

                        Ok(CpuCycleResult {
                            instruction_complete: true,
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "Implicit: dummy read, execute",
                            ..Default::default()
                        })
                    }
                }
            }

            // ========== Accumulator (2 cycles total) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Dummy read PC+1, execute
            Accumulator => {
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?; // dummy read

                if !is_interrupt {
                    let operand = Operand::Accumulator;
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "Accumulator: dummy read, execute",
                    ..Default::default()
                })
            }

            // ========== Immediate (2 cycles total) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch immediate value, execute
            Immediate => {
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                if !is_interrupt {
                    let operand = Operand::Byte(data);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "Immediate: fetch value, execute",
                    ..Default::default()
                })
            }

            // ========== Zero Page (3 cycles for read/write, 5 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch ZP address
            // Cycle 3: Read/Write from ZP
            // (RMW) Cycle 4: Dummy write original value
            // (RMW) Cycle 5: Write modified value
            ZeroPage => {
                self.execute_cycle_zero_page(instruction, cycle, state, is_interrupt)
            }

            // ========== Zero Page,X (4 cycles for read/write, 6 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch ZP address
            // Cycle 3: Dummy read from ZP while adding X
            // Cycle 4: Read/Write from (ZP + X) & 0xFF
            // (RMW) Cycle 5: Dummy write original value
            // (RMW) Cycle 6: Write modified value
            ZeroPageIndexedX => {
                self.execute_cycle_zero_page_indexed(instruction, cycle, state, is_interrupt, self.registers.x)
            }

            // ========== Zero Page,Y (4 cycles for read/write) ==========
            // Same as ZP,X but with Y register
            ZeroPageIndexedY => {
                self.execute_cycle_zero_page_indexed(instruction, cycle, state, is_interrupt, self.registers.y)
            }

            // ========== Absolute (4 cycles for read/write, 6 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch address low byte
            // Cycle 3: Fetch address high byte
            // Cycle 4: Read/Write from address
            // (RMW) Cycle 5: Dummy write original value
            // (RMW) Cycle 6: Write modified value
            Absolute => {
                self.execute_cycle_absolute(instruction, cycle, state, is_interrupt)
            }

            // ========== Absolute,X (4-5 cycles for read, 5 for write, 7 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch address low byte
            // Cycle 3: Fetch address high byte
            // Cycle 4: Read from (addr + X) - may be wrong page
            // Cycle 5: Read from correct address (if page crossed) OR always for write/RMW
            // (RMW) Cycle 6: Dummy write original value
            // (RMW) Cycle 7: Write modified value
            AbsoluteIndexedX => {
                self.execute_cycle_absolute_indexed(instruction, cycle, state, is_interrupt, self.registers.x)
            }

            // ========== Absolute,Y (4-5 cycles for read, 5 for write) ==========
            // Same as Absolute,X but with Y register
            AbsoluteIndexedY => {
                self.execute_cycle_absolute_indexed(instruction, cycle, state, is_interrupt, self.registers.y)
            }

            // ========== Relative (2-4 cycles for branches) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch offset
            // Cycle 3: (if taken) Fetch from wrong address while adding offset
            // Cycle 4: (if page crossed) Fetch from wrong address while fixing high byte
            Relative => {
                self.execute_cycle_relative(instruction, cycle, state, is_interrupt)
            }

            // ========== Indirect (JMP only, 5 cycles) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch pointer low byte
            // Cycle 3: Fetch pointer high byte
            // Cycle 4: Fetch effective address low byte
            // Cycle 5: Fetch effective address high byte (with page wrap bug)
            Indirect => {
                self.execute_cycle_indirect_jmp(instruction, cycle, state, is_interrupt)
            }

            // ========== (Indirect,X) (6 cycles for read/write, 8 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch pointer address
            // Cycle 3: Dummy read from pointer while adding X
            // Cycle 4: Fetch effective address low from (ptr + X) & 0xFF
            // Cycle 5: Fetch effective address high from (ptr + X + 1) & 0xFF
            // Cycle 6: Read/Write from effective address
            // (RMW) Cycle 7: Dummy write original value
            // (RMW) Cycle 8: Write modified value
            IndirectIndexedX => {
                self.execute_cycle_indirect_x(instruction, cycle, state, is_interrupt)
            }

            // ========== (Indirect),Y (5-6 cycles for read, 6 for write, 8 for RMW) ==========
            // Cycle 1: Fetch opcode (done)
            // Cycle 2: Fetch pointer address
            // Cycle 3: Fetch effective address low from ptr
            // Cycle 4: Fetch effective address high from (ptr + 1) & 0xFF
            // Cycle 5: Read from (effective + Y) - may be wrong page
            // Cycle 6: Read from correct address (if page crossed) OR always for write
            // (RMW) Cycle 7: Dummy write original value
            // (RMW) Cycle 8: Write modified value
            IndirectIndexedY => {
                self.execute_cycle_indirect_y(instruction, cycle, state, is_interrupt)
            }
        }
    }

    /// Execute cycle for Zero Page addressing mode
    fn execute_cycle_zero_page(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);

        match cycle {
            1 => {
                // Fetch ZP address
                let addr = pc.wrapping_add(1);
                let zp_addr = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = zp_addr;
                state.effective_addr = zp_addr as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(zp_addr),
                    bus_op: BusOperation::Read,
                    cycle_description: "ZP: fetch address",
                    ..Default::default()
                })
            }
            2 => {
                let addr = state.effective_addr;
                if is_write {
                    // Write instruction - write value to ZP
                    let value = self.get_write_value(instruction)?;
                    self.bus.borrow_mut().write_byte(addr, value)?;

                    if !is_interrupt {
                        // For write instructions, we don't call execute_instruction
                        // as the write IS the operation
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "ZP: write value",
                        ..Default::default()
                    })
                } else {
                    // Read/RMW - read from ZP
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    if is_rmw {
                        Ok(CpuCycleResult {
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "ZP: read value (RMW)",
                            ..Default::default()
                        })
                    } else {
                        // Read instruction - execute now
                        if !is_interrupt {
                            // Use AddressWithValue to avoid double-read in instruction executor
                            let operand = Operand::AddressWithValue(addr, data);
                            self.execute_instruction(&instruction, &operand)?;
                            self.finalize_instruction(instruction)?;
                        }

                        Ok(CpuCycleResult {
                            instruction_complete: true,
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "ZP: read value, execute",
                            ..Default::default()
                        })
                    }
                }
            }
            3 if is_rmw => {
                // RMW: Dummy write of original value
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "ZP RMW: dummy write",
                    ..Default::default()
                })
            }
            4 if is_rmw => {
                // RMW: Execute and write modified value
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressWithValue to pass the pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressWithValue(addr, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "ZP RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("ZP cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for Zero Page Indexed addressing mode
    fn execute_cycle_zero_page_indexed(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
        index: u8,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);

        match cycle {
            1 => {
                // Fetch ZP address
                let addr = pc.wrapping_add(1);
                let zp_addr = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = zp_addr;
                state.base_addr = zp_addr as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(zp_addr),
                    bus_op: BusOperation::Read,
                    cycle_description: "ZP,X/Y: fetch address",
                    ..Default::default()
                })
            }
            2 => {
                // Dummy read from unindexed ZP while adding index
                let addr = state.base_addr;
                let data = self.bus.borrow().read_byte(addr)?;
                state.effective_addr = state.operand_lo.wrapping_add(index) as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "ZP,X/Y: dummy read while indexing",
                    ..Default::default()
                })
            }
            3 => {
                let addr = state.effective_addr;
                if is_write {
                    // Write to indexed ZP (use base_addr for SHA/SHX/SHY)
                    let value = self.get_write_value_with_base(instruction, state.base_addr)?;
                    self.bus.borrow_mut().write_byte(addr, value)?;

                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "ZP,X/Y: write value",
                        ..Default::default()
                    })
                } else {
                    // Read from indexed ZP
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    if is_rmw {
                        Ok(CpuCycleResult {
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "ZP,X/Y: read value (RMW)",
                            ..Default::default()
                        })
                    } else {
                        if !is_interrupt {
                            // Use AddressEffectiveWithValue to avoid double-read
                            let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, false, data);
                            self.execute_instruction(&instruction, &operand)?;
                            self.finalize_instruction(instruction)?;
                        }

                        Ok(CpuCycleResult {
                            instruction_complete: true,
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "ZP,X/Y: read value, execute",
                            ..Default::default()
                        })
                    }
                }
            }
            4 if is_rmw => {
                // RMW: Dummy write of original value
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "ZP,X/Y RMW: dummy write",
                    ..Default::default()
                })
            }
            5 if is_rmw => {
                // RMW: Execute and write
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressEffectiveWithValue to pass pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, false, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "ZP,X/Y RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("ZP,X/Y cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for Absolute addressing mode
    fn execute_cycle_absolute(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);
        let is_jmp = instruction.opcode == OpCode::JMP;
        let is_jsr = instruction.opcode == OpCode::JSR;

        // JSR has special timing - handle separately
        if is_jsr {
            return self.execute_cycle_jsr(instruction, cycle, state, is_interrupt);
        }

        match cycle {
            1 => {
                // Fetch address low byte
                let addr = pc.wrapping_add(1);
                let lo = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = lo;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "Abs: fetch addr low",
                    ..Default::default()
                })
            }
            2 => {
                // Fetch address high byte
                let addr = pc.wrapping_add(2);
                let hi = self.bus.borrow().read_byte(addr)?;
                state.operand_hi = hi;
                state.effective_addr = (hi as u16) << 8 | state.operand_lo as u16;

                if is_jmp {
                    // JMP executes immediately after fetching address
                    if !is_interrupt {
                        self.registers.set_pc(state.effective_addr);
                        // Clear is_pc_dirty so the next instruction advances PC correctly
                        self.registers.is_pc_dirty = false;
                        self.instructions_executed += 1;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(hi),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs JMP: fetch addr high, jump",
                        ..Default::default()
                    })
                } else {
                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(addr),
                        data: Some(hi),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs: fetch addr high",
                        ..Default::default()
                    })
                }
            }
            3 => {
                let addr = state.effective_addr;
                if is_write {
                    let value = self.get_write_value(instruction)?;
                    self.bus.borrow_mut().write_byte(addr, value)?;

                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "Abs: write value",
                        ..Default::default()
                    })
                } else {
                    // Read
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    if is_rmw {
                        Ok(CpuCycleResult {
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "Abs: read value (RMW)",
                            ..Default::default()
                        })
                    } else {
                        if !is_interrupt {
                            // Use AddressWithValue to avoid double-read in instruction executor
                            let operand = Operand::AddressWithValue(addr, data);
                            self.execute_instruction(&instruction, &operand)?;
                            self.finalize_instruction(instruction)?;
                        }

                        Ok(CpuCycleResult {
                            instruction_complete: true,
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "Abs: read value, execute",
                            ..Default::default()
                        })
                    }
                }
            }
            4 if is_rmw => {
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "Abs RMW: dummy write",
                    ..Default::default()
                })
            }
            5 if is_rmw => {
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressWithValue to pass pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressWithValue(addr, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "Abs RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("Abs cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for JSR (special absolute timing)
    fn execute_cycle_jsr(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Fetch address low byte
                let addr = pc.wrapping_add(1);
                let lo = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = lo;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "JSR: fetch addr low",
                    ..Default::default()
                })
            }
            2 => {
                // Internal operation (dummy read of stack)
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let data = self.bus.borrow().read_byte(stack_addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "JSR: internal (stack read)",
                    ..Default::default()
                })
            }
            3 => {
                // Push PCH
                let return_addr = pc.wrapping_add(2); // PC + 2 (points to last byte of JSR instruction)
                let pch = (return_addr >> 8) as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pch)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pch),
                    bus_op: BusOperation::Write,
                    cycle_description: "JSR: push PCH",
                    ..Default::default()
                })
            }
            4 => {
                // Push PCL
                let return_addr = pc.wrapping_add(2);
                let pcl = return_addr as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pcl)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pcl),
                    bus_op: BusOperation::Write,
                    cycle_description: "JSR: push PCL",
                    ..Default::default()
                })
            }
            5 => {
                // Fetch address high byte and jump
                let addr = pc.wrapping_add(2);
                let hi = self.bus.borrow().read_byte(addr)?;
                state.operand_hi = hi;
                let target = (hi as u16) << 8 | state.operand_lo as u16;

                if !is_interrupt {
                    self.registers.set_pc(target);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "JSR: fetch addr high, jump",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("JSR cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for Absolute Indexed addressing mode
    fn execute_cycle_absolute_indexed(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
        index: u8,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);

        match cycle {
            1 => {
                // Fetch address low byte
                let addr = pc.wrapping_add(1);
                let lo = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = lo;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "Abs,X/Y: fetch addr low",
                    ..Default::default()
                })
            }
            2 => {
                // Fetch address high byte, calculate effective address
                let addr = pc.wrapping_add(2);
                let hi = self.bus.borrow().read_byte(addr)?;
                state.operand_hi = hi;
                state.base_addr = (hi as u16) << 8 | state.operand_lo as u16;
                state.effective_addr = state.base_addr.wrapping_add(index as u16);
                state.page_crossed = (state.base_addr & 0xFF00) != (state.effective_addr & 0xFF00);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "Abs,X/Y: fetch addr high",
                    ..Default::default()
                })
            }
            3 => {
                if is_write || is_rmw {
                    // Write/RMW always reads from possibly-wrong address
                    let wrong_addr = (state.base_addr & 0xFF00) | ((state.operand_lo.wrapping_add(index)) as u16);
                    let data = self.bus.borrow().read_byte(wrong_addr)?;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(wrong_addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs,X/Y: read (fix high byte)",
                        ..Default::default()
                    })
                } else if state.page_crossed {
                    // Read with page crossing - dummy read from wrong address
                    let wrong_addr = (state.base_addr & 0xFF00) | ((state.operand_lo.wrapping_add(index)) as u16);
                    let data = self.bus.borrow().read_byte(wrong_addr)?;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(wrong_addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs,X/Y: dummy read (page cross)",
                        ..Default::default()
                    })
                } else {
                    // Read without page crossing - execute on this cycle
                    let addr = state.effective_addr;
                    let data = self.bus.borrow().read_byte(addr)?;

                    if !is_interrupt {
                        // Use AddressEffectiveWithValue to avoid double-read
                        let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, false, data);
                        self.execute_instruction(&instruction, &operand)?;
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs,X/Y: read, execute",
                        ..Default::default()
                    })
                }
            }
            4 => {
                let addr = state.effective_addr;

                if is_write {
                    // Write to effective address (use base_addr for SHA/SHX/SHY)
                    let value = self.get_write_value_with_base(instruction, state.base_addr)?;

                    // TAS (OpCode::TAX) also sets SP = A & X before the store
                    if instruction.opcode == OpCode::TAX {
                        self.registers.sp = self.registers.a & self.registers.x;
                    }

                    // SHA/SHX/SHY/TAS with page crossing corrupt the target address
                    let target_addr = if self.is_unstable_store(instruction) && state.page_crossed {
                        self.get_corrupted_address(addr, value)
                    } else {
                        addr
                    };

                    self.bus.borrow_mut().write_byte(target_addr, value)?;

                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(target_addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "Abs,X/Y: write value",
                        ..Default::default()
                    })
                } else if is_rmw {
                    // RMW: Read from effective address
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs,X/Y RMW: read value",
                        ..Default::default()
                    })
                } else {
                    // Read with page cross - now read from correct address
                    let data = self.bus.borrow().read_byte(addr)?;

                    if !is_interrupt {
                        // Use AddressEffectiveWithValue to avoid double-read
                        let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, true, data);
                        self.execute_instruction(&instruction, &operand)?;
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Abs,X/Y: read correct addr, execute",
                        ..Default::default()
                    })
                }
            }
            5 if is_rmw => {
                // RMW: Dummy write
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "Abs,X/Y RMW: dummy write",
                    ..Default::default()
                })
            }
            6 if is_rmw => {
                // RMW: Write result
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressEffectiveWithValue to pass pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, state.page_crossed, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "Abs,X/Y RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("Abs,X/Y cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for Relative (branch) addressing mode
    fn execute_cycle_relative(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Fetch offset
                let addr = pc.wrapping_add(1);
                let offset = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = offset;

                // Calculate target address
                let next_pc = pc.wrapping_add(2); // PC after branch instruction
                let target = next_pc.wrapping_add_signed(offset as i8 as i16);
                state.branch_target = target;
                state.page_crossed = (next_pc & 0xFF00) != (target & 0xFF00);

                // Evaluate branch condition
                state.branch_taken = self.evaluate_branch_condition(instruction)?;

                if !state.branch_taken {
                    // Branch not taken - instruction complete
                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(offset),
                        bus_op: BusOperation::Read,
                        cycle_description: "Branch: fetch offset (not taken)",
                        ..Default::default()
                    })
                } else {
                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(addr),
                        data: Some(offset),
                        bus_op: BusOperation::Read,
                        cycle_description: "Branch: fetch offset (taken)",
                        ..Default::default()
                    })
                }
            }
            2 => {
                // Branch taken - fetch from old PC+2 while adding offset
                let addr = pc.wrapping_add(2);
                let data = self.bus.borrow().read_byte(addr)?;

                if !state.page_crossed {
                    // No page cross - branch completes
                    if !is_interrupt {
                        self.registers.set_pc(state.branch_target);
                        // Clear is_pc_dirty so the next instruction advances PC correctly
                        self.registers.is_pc_dirty = false;
                        self.instructions_executed += 1;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Branch: taken (no page cross)",
                        ..Default::default()
                    })
                } else {
                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "Branch: taken (page cross pending)",
                        ..Default::default()
                    })
                }
            }
            3 => {
                // Page crossed - extra cycle to fix high byte
                // Dummy read from wrong address (same low byte, wrong high byte)
                let wrong_addr = (pc.wrapping_add(2) & 0xFF00) | (state.branch_target & 0x00FF);
                let data = self.bus.borrow().read_byte(wrong_addr)?;

                if !is_interrupt {
                    self.registers.set_pc(state.branch_target);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(wrong_addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "Branch: fix page crossing",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("Relative cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for Indirect JMP
    fn execute_cycle_indirect_jmp(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Fetch pointer low byte
                let addr = pc.wrapping_add(1);
                let lo = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = lo;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "JMP ind: fetch ptr low",
                    ..Default::default()
                })
            }
            2 => {
                // Fetch pointer high byte
                let addr = pc.wrapping_add(2);
                let hi = self.bus.borrow().read_byte(addr)?;
                state.operand_hi = hi;
                state.base_addr = (hi as u16) << 8 | state.operand_lo as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "JMP ind: fetch ptr high",
                    ..Default::default()
                })
            }
            3 => {
                // Fetch effective address low byte
                let addr = state.base_addr;
                let lo = self.bus.borrow().read_byte(addr)?;
                state.effective_addr = lo as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "JMP ind: fetch eff low",
                    ..Default::default()
                })
            }
            4 => {
                // Fetch effective address high byte (with page wrap bug)
                let ptr_lo = state.base_addr as u8;
                let addr = (state.base_addr & 0xFF00) | ptr_lo.wrapping_add(1) as u16;
                let hi = self.bus.borrow().read_byte(addr)?;
                state.effective_addr |= (hi as u16) << 8;

                if !is_interrupt {
                    self.registers.set_pc(state.effective_addr);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "JMP ind: fetch eff high, jump",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("JMP ind cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for (Indirect,X) addressing mode
    fn execute_cycle_indirect_x(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let x = self.registers.x;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);

        match cycle {
            1 => {
                // Fetch pointer address
                let addr = pc.wrapping_add(1);
                let ptr = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = ptr;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(ptr),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind,X): fetch ptr",
                    ..Default::default()
                })
            }
            2 => {
                // Dummy read from pointer while adding X
                let addr = state.operand_lo as u16;
                let data = self.bus.borrow().read_byte(addr)?;
                state.base_addr = state.operand_lo.wrapping_add(x) as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind,X): dummy read (adding X)",
                    ..Default::default()
                })
            }
            3 => {
                // Fetch effective address low byte
                let addr = state.base_addr;
                let lo = self.bus.borrow().read_byte(addr)?;
                state.effective_addr = lo as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind,X): fetch eff low",
                    ..Default::default()
                })
            }
            4 => {
                // Fetch effective address high byte
                let addr = (state.base_addr as u8).wrapping_add(1) as u16;
                let hi = self.bus.borrow().read_byte(addr)?;
                state.effective_addr |= (hi as u16) << 8;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind,X): fetch eff high",
                    ..Default::default()
                })
            }
            5 => {
                let addr = state.effective_addr;

                if is_write {
                    // Use effective address high byte for SHA/SHX/SHY (Indirect X uses final address)
                    let value = self.get_write_value_with_base(instruction, state.effective_addr)?;
                    self.bus.borrow_mut().write_byte(addr, value)?;

                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "(Ind,X): write value",
                        ..Default::default()
                    })
                } else {
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    if is_rmw {
                        Ok(CpuCycleResult {
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "(Ind,X) RMW: read value",
                            ..Default::default()
                        })
                    } else {
                        if !is_interrupt {
                            // Use AddressEffectiveWithValue to avoid double-read
                            let operand = Operand::AddressEffectiveWithValue(state.operand_lo as u16, addr, false, data);
                            self.execute_instruction(&instruction, &operand)?;
                            self.finalize_instruction(instruction)?;
                        }

                        Ok(CpuCycleResult {
                            instruction_complete: true,
                            memory_read: true,
                            address: Some(addr),
                            data: Some(data),
                            bus_op: BusOperation::Read,
                            cycle_description: "(Ind,X): read, execute",
                            ..Default::default()
                        })
                    }
                }
            }
            6 if is_rmw => {
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "(Ind,X) RMW: dummy write",
                    ..Default::default()
                })
            }
            7 if is_rmw => {
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressEffectiveWithValue to pass pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressEffectiveWithValue(state.operand_lo as u16, addr, false, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "(Ind,X) RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("(Ind,X) cycle {} unexpected", cycle)))
            }
        }
    }

    /// Execute cycle for (Indirect),Y addressing mode
    fn execute_cycle_indirect_y(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;
        let y = self.registers.y;
        let is_rmw = self.is_rmw_instruction(instruction);
        let is_write = self.is_write_instruction(instruction);

        match cycle {
            1 => {
                // Fetch pointer address
                let addr = pc.wrapping_add(1);
                let ptr = self.bus.borrow().read_byte(addr)?;
                state.operand_lo = ptr;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(ptr),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind),Y: fetch ptr",
                    ..Default::default()
                })
            }
            2 => {
                // Fetch effective address low byte from ptr
                let addr = state.operand_lo as u16;
                let lo = self.bus.borrow().read_byte(addr)?;
                state.effective_addr = lo as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind),Y: fetch eff low",
                    ..Default::default()
                })
            }
            3 => {
                // Fetch effective address high byte from (ptr+1) & 0xFF
                let addr = state.operand_lo.wrapping_add(1) as u16;
                let hi = self.bus.borrow().read_byte(addr)?;
                state.base_addr = (hi as u16) << 8 | state.effective_addr;
                state.effective_addr = state.base_addr.wrapping_add(y as u16);
                state.page_crossed = (state.base_addr & 0xFF00) != (state.effective_addr & 0xFF00);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "(Ind),Y: fetch eff high",
                    ..Default::default()
                })
            }
            4 => {
                if is_write || is_rmw {
                    // Write/RMW always reads from possibly-wrong address
                    let base_lo = state.base_addr as u8;
                    let wrong_addr = (state.base_addr & 0xFF00) | base_lo.wrapping_add(y) as u16;
                    let data = self.bus.borrow().read_byte(wrong_addr)?;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(wrong_addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "(Ind),Y: fix high byte",
                        ..Default::default()
                    })
                } else if state.page_crossed {
                    // Page crossed - dummy read from wrong address
                    let base_lo = state.base_addr as u8;
                    let wrong_addr = (state.base_addr & 0xFF00) | base_lo.wrapping_add(y) as u16;
                    let data = self.bus.borrow().read_byte(wrong_addr)?;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(wrong_addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "(Ind),Y: dummy read (page cross)",
                        ..Default::default()
                    })
                } else {
                    // No page crossing - execute
                    let addr = state.effective_addr;
                    let data = self.bus.borrow().read_byte(addr)?;

                    if !is_interrupt {
                        // Use AddressEffectiveWithValue to avoid double-read
                        let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, false, data);
                        self.execute_instruction(&instruction, &operand)?;
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "(Ind),Y: read, execute",
                        ..Default::default()
                    })
                }
            }
            5 => {
                let addr = state.effective_addr;

                if is_write {
                    // Use base_addr for SHA/SHX/SHY (address before Y indexing)
                    let value = self.get_write_value_with_base(instruction, state.base_addr)?;

                    // SHA/SHX/SHY with page crossing corrupt the target address
                    let target_addr = if self.is_unstable_store(instruction) && state.page_crossed {
                        self.get_corrupted_address(addr, value)
                    } else {
                        addr
                    };

                    self.bus.borrow_mut().write_byte(target_addr, value)?;

                    if !is_interrupt {
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_write: true,
                        address: Some(target_addr),
                        data: Some(value),
                        bus_op: BusOperation::Write,
                        cycle_description: "(Ind),Y: write value",
                        ..Default::default()
                    })
                } else if is_rmw {
                    // Read value for RMW
                    let data = self.bus.borrow().read_byte(addr)?;
                    state.read_value = data;

                    Ok(CpuCycleResult {
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "(Ind),Y RMW: read value",
                        ..Default::default()
                    })
                } else {
                    // Read after page cross
                    let data = self.bus.borrow().read_byte(addr)?;

                    if !is_interrupt {
                        // Use AddressEffectiveWithValue to avoid double-read
                        let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, true, data);
                        self.execute_instruction(&instruction, &operand)?;
                        self.finalize_instruction(instruction)?;
                    }

                    Ok(CpuCycleResult {
                        instruction_complete: true,
                        memory_read: true,
                        address: Some(addr),
                        data: Some(data),
                        bus_op: BusOperation::Read,
                        cycle_description: "(Ind),Y: read correct, execute",
                        ..Default::default()
                    })
                }
            }
            6 if is_rmw => {
                let addr = state.effective_addr;
                let value = state.read_value;
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "(Ind),Y RMW: dummy write",
                    ..Default::default()
                })
            }
            7 if is_rmw => {
                let addr = state.effective_addr;

                if !is_interrupt {
                    // Use AddressEffectiveWithValue to pass pre-read value for RMW
                    // This makes rmw_overwrite store result in rmw_pending_value instead of writing
                    let operand = Operand::AddressEffectiveWithValue(state.base_addr, addr, state.page_crossed, state.read_value);
                    self.execute_instruction(&instruction, &operand)?;
                    self.finalize_instruction(instruction)?;
                }

                // Get the modified value from rmw_pending_value (set by rmw_overwrite)
                let value = self.rmw_pending_value.take().unwrap_or(state.read_value);

                // Write the modified value to bus
                self.bus.borrow_mut().write_byte(addr, value)?;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "(Ind),Y RMW: write result",
                    ..Default::default()
                })
            }
            _ => {
                Err(CpuError::Unimplemented(format!("(Ind),Y cycle {} unexpected", cycle)))
            }
        }
    }

    /// Check if instruction is a Read-Modify-Write operation
    fn is_rmw_instruction(&self, instruction: &Instruction) -> bool {
        matches!(instruction.opcode,
            OpCode::ASL | OpCode::LSR | OpCode::ROL | OpCode::ROR |
            OpCode::INC | OpCode::DEC |
            // Illegal RMW opcodes
            OpCode::SLO | OpCode::SRE | OpCode::RLA | OpCode::RRA |
            OpCode::ISB | OpCode::DCP
        )
    }

    /// Check if instruction is a write-only operation (STA, STX, STY, SAX, etc.)
    fn is_write_instruction(&self, instruction: &Instruction) -> bool {
        matches!(instruction.opcode,
            OpCode::STA | OpCode::STX | OpCode::STY |
            // Illegal write opcodes
            // Note: TAX is used for TAS (0x9B) which stores A & X & (H+1)
            OpCode::SAX | OpCode::SHA | OpCode::SHX | OpCode::SHY | OpCode::TAX
        )
    }

    /// Get the value to write for a store instruction
    /// For SHA/SHX/SHY, base_addr is needed to compute the correct value.
    fn get_write_value(&self, instruction: &Instruction) -> Result<u8, CpuError> {
        self.get_write_value_with_base(instruction, 0)
    }

    /// Get the value to write for a store instruction, with base address for indexed modes
    /// SHA, SHX, SHY use formula: register & (H+1) where H is high byte of base address
    fn get_write_value_with_base(&self, instruction: &Instruction, base_addr: u16) -> Result<u8, CpuError> {
        let h = (base_addr >> 8) as u8;
        match instruction.opcode {
            OpCode::STA => Ok(self.registers.a),
            OpCode::STX => Ok(self.registers.x),
            OpCode::STY => Ok(self.registers.y),
            OpCode::SAX => Ok(self.registers.a & self.registers.x),
            // SHA: A & X & (H+1) where H is high byte of base address
            OpCode::SHA => Ok(self.registers.a & self.registers.x & h.wrapping_add(1)),
            // SHX: X & (H+1)
            OpCode::SHX => Ok(self.registers.x & h.wrapping_add(1)),
            // SHY: Y & (H+1)
            OpCode::SHY => Ok(self.registers.y & h.wrapping_add(1)),
            // TAX (TAS): A & X & (H+1) - same value as SHA
            OpCode::TAX => Ok(self.registers.a & self.registers.x & h.wrapping_add(1)),
            _ => Err(CpuError::Unimplemented(format!("get_write_value for {:?}", instruction.opcode)))
        }
    }

    /// Check if instruction is an unstable store (SHA/SHX/SHY) that corrupts address on page crossing
    fn is_unstable_store(&self, instruction: &Instruction) -> bool {
        // SHA, SHX, SHY, and TAX (TAS) are unstable store instructions
        // On page crossing, the address high byte gets corrupted
        matches!(instruction.opcode, OpCode::SHA | OpCode::SHX | OpCode::SHY | OpCode::TAX)
    }

    /// Get the corrupted address for SHA/SHX/SHY when page crossing occurs
    /// The address becomes: low byte from effective address, high byte from value stored
    fn get_corrupted_address(&self, effective_addr: u16, value: u8) -> u16 {
        (effective_addr & 0x00FF) | ((value as u16) << 8)
    }

    /// Evaluate branch condition for branch instructions
    fn evaluate_branch_condition(&self, instruction: &Instruction) -> Result<bool, CpuError> {
        let taken = match instruction.opcode {
            OpCode::BCC => !self.registers.get_status(StatusFlag::Carry),
            OpCode::BCS => self.registers.get_status(StatusFlag::Carry),
            OpCode::BEQ => self.registers.get_status(StatusFlag::Zero),
            OpCode::BMI => self.registers.get_status(StatusFlag::Negative),
            OpCode::BNE => !self.registers.get_status(StatusFlag::Zero),
            OpCode::BPL => !self.registers.get_status(StatusFlag::Negative),
            OpCode::BVC => !self.registers.get_status(StatusFlag::Overflow),
            OpCode::BVS => self.registers.get_status(StatusFlag::Overflow),
            _ => return Err(CpuError::Unimplemented(format!("Branch condition for {:?}", instruction.opcode)))
        };
        Ok(taken)
    }

    /// Finalize instruction - update PC and increment instruction counter
    fn finalize_instruction(&mut self, instruction: &Instruction) -> Result<(), CpuError> {
        // Update PC if not already modified by the instruction
        if !self.registers.is_pc_dirty {
            self.registers.pc = self.registers.safe_pc_add(instruction.bytes as i16)?;
        } else {
            self.registers.is_pc_dirty = false;
        }

        self.instructions_executed += 1;

        // Handle pending I flag changes
        if let Some(new_i_flag) = self.pending_i_flag.take() {
            self.registers.set_status(StatusFlag::InterruptDisable, new_i_flag);
        }

        Ok(())
    }

    /// Execute cycle for PHA/PHP (3 cycles total)
    /// Cycle 1: Fetch opcode (done)
    /// Cycle 2: Dummy read from PC+1
    /// Cycle 3: Write A/P to stack, decrement SP
    fn execute_cycle_push_stack(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        _state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Dummy read from PC+1
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "PHA/PHP: dummy read",
                    ..Default::default()
                })
            }
            2 => {
                // Write to stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let value = match instruction.opcode {
                    OpCode::PHA => self.registers.a,
                    OpCode::PHP => self.registers.p | 0x30, // Set B and unused flags
                    _ => unreachable!()
                };
                self.bus.borrow_mut().write_byte(stack_addr, value)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                if !is_interrupt {
                    self.finalize_instruction(instruction)?;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(value),
                    bus_op: BusOperation::Write,
                    cycle_description: "PHA/PHP: write to stack",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("PHA/PHP cycle {} unexpected", cycle)))
        }
    }

    /// Execute cycle for PLA/PLP (4 cycles total)
    /// Cycle 1: Fetch opcode (done)
    /// Cycle 2: Dummy read from PC+1
    /// Cycle 3: Dummy read from stack pointer (increment SP)
    /// Cycle 4: Read from new stack location
    fn execute_cycle_pull_stack(
        &mut self,
        instruction: &'static Instruction,
        cycle: u8,
        _state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Dummy read from PC+1
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "PLA/PLP: dummy read",
                    ..Default::default()
                })
            }
            2 => {
                // Dummy read from current stack pointer (while incrementing)
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let data = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "PLA/PLP: dummy stack read",
                    ..Default::default()
                })
            }
            3 => {
                // Read from new stack location
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let value = self.bus.borrow().read_byte(stack_addr)?;

                if !is_interrupt {
                    match instruction.opcode {
                        OpCode::PLA => {
                            self.registers.a = value;
                            self.update_flags_zero_negative(value);
                        }
                        OpCode::PLP => {
                            // Ignore B flag and unused bit when pulling
                            self.registers.p = (value & 0xCF) | (self.registers.p & 0x30);
                        }
                        _ => unreachable!()
                    }
                    self.finalize_instruction(instruction)?;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(value),
                    bus_op: BusOperation::Read,
                    cycle_description: "PLA/PLP: read from stack",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("PLA/PLP cycle {} unexpected", cycle)))
        }
    }

    /// Execute cycle for RTS (6 cycles total)
    /// Cycle 1: Fetch opcode (done)
    /// Cycle 2: Dummy read from PC+1
    /// Cycle 3: Dummy read from stack (increment SP)
    /// Cycle 4: Read PCL from stack
    /// Cycle 5: Read PCH from stack
    /// Cycle 6: Increment PC (internal)
    fn execute_cycle_rts(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Dummy read from PC+1
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTS: dummy read",
                    ..Default::default()
                })
            }
            2 => {
                // Dummy read from stack (while incrementing)
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let data = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTS: dummy stack read",
                    ..Default::default()
                })
            }
            3 => {
                // Read PCL from stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let pcl = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);
                state.operand_lo = pcl;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(pcl),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTS: read PCL",
                    ..Default::default()
                })
            }
            4 => {
                // Read PCH from stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let pch = self.bus.borrow().read_byte(stack_addr)?;
                state.operand_hi = pch;
                state.effective_addr = ((pch as u16) << 8) | state.operand_lo as u16;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(pch),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTS: read PCH",
                    ..Default::default()
                })
            }
            5 => {
                // Increment PC (internal, dummy read from return address)
                let addr = state.effective_addr;
                let data = self.bus.borrow().read_byte(addr)?;
                let new_pc = state.effective_addr.wrapping_add(1);

                if !is_interrupt {
                    self.registers.set_pc(new_pc);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTS: increment PC",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("RTS cycle {} unexpected", cycle)))
        }
    }

    /// Execute cycle for RTI (6 cycles total)
    /// Cycle 1: Fetch opcode (done)
    /// Cycle 2: Dummy read from PC+1
    /// Cycle 3: Dummy read from stack (increment SP)
    /// Cycle 4: Read P from stack
    /// Cycle 5: Read PCL from stack
    /// Cycle 6: Read PCH from stack
    fn execute_cycle_rti(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Dummy read from PC+1
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTI: dummy read",
                    ..Default::default()
                })
            }
            2 => {
                // Dummy read from stack (while incrementing)
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let data = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTI: dummy stack read",
                    ..Default::default()
                })
            }
            3 => {
                // Read P from stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let p = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);

                if !is_interrupt {
                    // Ignore B flag and set unused bit when pulling
                    self.registers.p = (p & 0xCF) | 0x20;
                }
                state.read_value = p;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(p),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTI: read P",
                    ..Default::default()
                })
            }
            4 => {
                // Read PCL from stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let pcl = self.bus.borrow().read_byte(stack_addr)?;
                self.registers.sp = self.registers.sp.wrapping_add(1);
                state.operand_lo = pcl;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(pcl),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTI: read PCL",
                    ..Default::default()
                })
            }
            5 => {
                // Read PCH from stack
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                let pch = self.bus.borrow().read_byte(stack_addr)?;
                let new_pc = ((pch as u16) << 8) | state.operand_lo as u16;

                if !is_interrupt {
                    self.registers.set_pc(new_pc);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(stack_addr),
                    data: Some(pch),
                    bus_op: BusOperation::Read,
                    cycle_description: "RTI: read PCH, set PC",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("RTI cycle {} unexpected", cycle)))
        }
    }

    /// Execute cycle for BRK (7 cycles total)
    /// Cycle 1: Fetch opcode (done)
    /// Cycle 2: Read and discard PC+1 (padding byte)
    /// Cycle 3: Push PCH to stack
    /// Cycle 4: Push PCL to stack
    /// Cycle 5: Push P to stack (with B flag set)
    /// Cycle 6: Read IRQ vector low
    /// Cycle 7: Read IRQ vector high
    fn execute_cycle_brk(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Read and discard padding byte
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "BRK: read padding",
                    ..Default::default()
                })
            }
            2 => {
                // Push PCH to stack
                let return_addr = pc.wrapping_add(2); // PC + 2 for BRK
                let pch = (return_addr >> 8) as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pch)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pch),
                    bus_op: BusOperation::Write,
                    cycle_description: "BRK: push PCH",
                    ..Default::default()
                })
            }
            3 => {
                // Push PCL to stack
                let return_addr = pc.wrapping_add(2);
                let pcl = return_addr as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pcl)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pcl),
                    bus_op: BusOperation::Write,
                    cycle_description: "BRK: push PCL",
                    ..Default::default()
                })
            }
            4 => {
                // Push P to stack with B flag set
                let p = self.registers.p | 0x30; // Set B and unused flags
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, p)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(p),
                    bus_op: BusOperation::Write,
                    cycle_description: "BRK: push P",
                    ..Default::default()
                })
            }
            5 => {
                // Read IRQ vector low
                let vector_addr = BRK_VECTOR;
                let vector_lo = self.bus.borrow().read_byte(vector_addr)?;
                state.operand_lo = vector_lo;

                // Set interrupt disable flag
                self.registers.set_status(StatusFlag::InterruptDisable, true);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(vector_addr),
                    data: Some(vector_lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "BRK: read vector low",
                    ..Default::default()
                })
            }
            6 => {
                // Read IRQ vector high
                let vector_addr = BRK_VECTOR.wrapping_add(1);
                let vector_hi = self.bus.borrow().read_byte(vector_addr)?;
                let new_pc = ((vector_hi as u16) << 8) | state.operand_lo as u16;

                if !is_interrupt {
                    self.registers.set_pc(new_pc);
                    // Clear is_pc_dirty so the next instruction advances PC correctly
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(vector_addr),
                    data: Some(vector_hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "BRK: read vector high, jump",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("BRK cycle {} unexpected", cycle)))
        }
    }

    /// Execute JAM/KIL/HLT instruction cycle by cycle.
    /// JAM freezes the CPU but performs specific bus activity:
    /// - Cycle 1: Fetch opcode (already done)
    /// - Cycle 2: Dummy read PC+1
    /// - Cycle 3: Read $FFFF
    /// - Cycle 4: Read $FFFE
    /// - Cycle 5: Read $FFFE
    /// - Cycles 6-11: Read $FFFF (6 times)
    fn execute_cycle_jam(
        &mut self,
        _instruction: &'static Instruction,
        cycle: u8,
        _state: &mut InstructionState,
        is_interrupt: bool,
    ) -> Result<CpuCycleResult, CpuError> {
        let pc = self.registers.pc;

        match cycle {
            1 => {
                // Dummy read PC+1
                let addr = pc.wrapping_add(1);
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "JAM: dummy read PC+1",
                    ..Default::default()
                })
            }
            2 => {
                // Read $FFFF
                let addr = 0xFFFF;
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "JAM: read $FFFF",
                    ..Default::default()
                })
            }
            3 => {
                // Read $FFFE
                let addr = 0xFFFE;
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "JAM: read $FFFE",
                    ..Default::default()
                })
            }
            4 => {
                // Read $FFFE again
                let addr = 0xFFFE;
                let data = self.bus.borrow().read_byte(addr)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "JAM: read $FFFE (2)",
                    ..Default::default()
                })
            }
            5..=10 => {
                // Read $FFFF (cycles 6-11 of the instruction)
                let addr = 0xFFFF;
                let data = self.bus.borrow().read_byte(addr)?;
                let is_last = cycle == 10;

                // On the last cycle, finalize the instruction
                if is_last && !is_interrupt {
                    // JAM just advances PC by 1 (the opcode byte)
                    self.registers.set_pc(pc.wrapping_add(1));
                    self.registers.is_pc_dirty = false;
                    self.instructions_executed += 1;
                }

                Ok(CpuCycleResult {
                    instruction_complete: is_last,
                    memory_read: true,
                    address: Some(addr),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: if is_last {
                        "JAM: read $FFFF (last)"
                    } else {
                        "JAM: read $FFFF"
                    },
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("JAM cycle {} unexpected", cycle)))
        }
    }

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
            nmi_line_low: false,
            pending_nmi: false,
            pending_i_flag: None,
            cycles: 0,
            cycle_state: CpuCycleState::FetchOpcode,
            latched_nmi: false,
            latched_irq: false,
            pending_interrupt: None,
            rmw_pending_value: None,
        }
    }

    const INTERRUPT_HANDLER_NUM_CYCLES: u32 = 7;

    /// DEPRECATED: Legacy synchronous interrupt handler.
    /// Use the cycle-stepped InterruptSequence state instead.
    /// This method runs the entire interrupt sequence synchronously, which breaks
    /// cycle-accurate timing. Kept only for reference/testing.
    #[deprecated(note = "Use InterruptSequence state machine for cycle-accurate interrupts")]
    #[allow(dead_code)]
    fn interrupt(&mut self) -> Result<u32, CpuError> {
        let cycle = if self.pending_nmi {
            self.pending_nmi = false;
            self.nmi()?;
            Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES
        } else if self.is_asserted_irq()? && !self.registers.get_status(StatusFlag::InterruptDisable) {
            self.irq()?;
            Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES
        } else {
            0
        };

        Ok(cycle)
    }

    /// DEPRECATED: Calculate additional cycles for an instruction.
    /// This was used by legacy instruction-level stepping which has been removed.
    /// The cycle-stepped state machine handles cycles internally per-cycle.
    #[deprecated(note = "Legacy method - cycle-stepped execution handles cycles per-cycle")]
    #[allow(dead_code)]
    fn calculate_additional_cycles(&self, instruction: &Instruction, operand: &Operand) -> u32 {
        match instruction.opcode {
            // Branch instructions: +1 if taken, +2 if taken and page crossed
            OpCode::BCC | OpCode::BCS | OpCode::BEQ | OpCode::BMI |
            OpCode::BNE | OpCode::BPL | OpCode::BVC | OpCode::BVS => {
                // For branches, we can't know if they're taken until execution
                // But we need to estimate. Return 0 here; actual cycles are handled in step_instruction
                0
            }

            // Load instructions with page crossing penalty
            OpCode::LDA | OpCode::LDX | OpCode::LDY | OpCode::EOR | OpCode::AND |
            OpCode::ORA | OpCode::ADC | OpCode::SBC | OpCode::CMP | OpCode::BIT |
            OpCode::LAX | OpCode::NOP | OpCode::LAS => {
                self.get_cycles_by_page_crossing_for_load(operand)
            }

            // All other instructions don't have additional cycles from operand
            _ => 0,
        }
    }

    /// DEPRECATED: Legacy synchronous interrupt check and setup.
    /// Use the cycle-stepped InterruptSequence state machine instead.
    ///
    /// Returns the number of cycles the interrupt will take (0 if no interrupt).
    /// NOTE: This uses the CURRENT interrupt state, not the latched state.
    /// For cycle-accurate emulation, the interrupt polling/latching happens per-cycle.
    #[deprecated(note = "Use InterruptSequence state machine for cycle-accurate interrupts")]
    #[allow(dead_code)]
    fn check_and_setup_interrupt(&mut self) -> Result<u32, CpuError> {
        if self.pending_nmi {
            self.pending_nmi = false;
            self.nmi()?;
            // Interrupt handling is now complete, cycles already counted
            // Return the cycle count but don't setup executing state
            // (the interrupt handler runs synchronously in nmi()/irq())
            Ok(Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES)
        } else if self.is_asserted_irq()? && !self.registers.get_status(StatusFlag::InterruptDisable) {
            self.irq()?;
            Ok(Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES)
        } else {
            Ok(0)
        }
    }

    /// Poll interrupt lines and update the latched state.
    /// This should be called at the START of each CPU cycle.
    ///
    /// NMI edge detection: NMI is edge-sensitive (falling edge). We detect when
    /// the NMI line transitions from high to low. Once detected, pending_nmi
    /// stays true until serviced, but we only latch it if it was pending at poll time.
    ///
    /// IRQ level detection: IRQ is level-sensitive. We sample the IRQ line state
    /// independently of the I flag. The I flag is checked only when deciding
    /// whether to service the interrupt (in check_interrupt_from_latch).
    fn poll_interrupts(&mut self) {
        // NMI: Latch if pending_nmi is currently set
        // Note: pending_nmi is set by signal_nmi() on the falling edge
        // The poll just samples whether it's currently pending
        if self.pending_nmi {
            self.latched_nmi = true;
        }

        // IRQ: Latch if IRQ line is asserted (independently of I flag)
        // The I flag is checked only when deciding whether to SERVICE the interrupt,
        // not when latching. This ensures correct behavior when:
        //   - IRQ is asserted while I=1
        //   - I is cleared shortly before the instruction boundary
        // Note: is_asserted_irq() returns Ok(true) if any IRQ source is active
        if let Ok(irq_asserted) = self.is_asserted_irq() {
            if irq_asserted {
                self.latched_irq = true;
            }
        }
    }

    /// Check for pending interrupts using the LATCHED state from polling.
    /// Returns the type of interrupt to service, or None if no interrupt pending.
    /// This does NOT execute the interrupt - it just determines which one (if any) should run.
    fn check_interrupt_from_latch(&self) -> Option<InterruptType> {
        // NMI has priority over IRQ
        if self.latched_nmi {
            Some(InterruptType::Nmi)
        } else if self.latched_irq {
            // Re-check the I flag - it could have been set during the instruction
            // (e.g., SEI instruction). The latched_irq indicates IRQ was pending
            // when polled, but the I flag state at completion determines if we service it.
            if !self.registers.get_status(StatusFlag::InterruptDisable) {
                Some(InterruptType::Irq)
            } else {
                None
            }
        } else {
            None
        }
    }

    /// Execute one cycle of a hardware interrupt sequence (NMI or IRQ).
    ///
    /// This handles cycles 2-7 of the interrupt sequence. Cycle 1 is performed
    /// in FetchOpcode (dummy read at PC before entering InterruptSequence).
    ///
    /// Cycle breakdown:
    /// - Cycle 2: Internal operation (dummy read from PC)
    /// - Cycle 3: Push PCH to stack
    /// - Cycle 4: Push PCL to stack
    /// - Cycle 5: Push P to stack (B flag clear for hardware interrupts)
    /// - Cycle 6: Read interrupt vector low byte, set I flag
    /// - Cycle 7: Read interrupt vector high byte, jump to handler
    ///
    /// The vector is passed in from the InterruptSequence state. It is set
    /// when entering the sequence and updated if NMI hijacks an IRQ sequence
    /// during cycles 1-4. This ensures consistent vector addressing.
    fn execute_interrupt_cycle(
        &mut self,
        cycle: u8,
        vector: u16,
        state: &mut InstructionState,
    ) -> Result<CpuCycleResult, CpuError> {
        debug_assert!(cycle >= 2 && cycle <= 7, "InterruptSequence cycles are 2-7, got {}", cycle);
        let pc = self.registers.pc;

        match cycle {
            2 => {
                // Internal operation - dummy read from current PC
                let data = self.bus.borrow().read_byte(pc)?;

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(pc),
                    data: Some(data),
                    bus_op: BusOperation::Read,
                    cycle_description: "Interrupt: internal op (read PC)",
                    ..Default::default()
                })
            }
            3 => {
                // Push PCH to stack
                let pch = (pc >> 8) as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pch)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pch),
                    bus_op: BusOperation::Write,
                    cycle_description: "Interrupt: push PCH",
                    ..Default::default()
                })
            }
            4 => {
                // Push PCL to stack
                let pcl = pc as u8;
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, pcl)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(pcl),
                    bus_op: BusOperation::Write,
                    cycle_description: "Interrupt: push PCL",
                    ..Default::default()
                })
            }
            5 => {
                // Push P to stack (B flag clear for hardware interrupts, unused set)
                let p = (self.registers.p & !StatusFlag::BreakCommand.bits()) | StatusFlag::Unused.bits();
                let stack_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;
                self.bus.borrow_mut().write_byte(stack_addr, p)?;
                self.registers.sp = self.registers.sp.wrapping_sub(1);

                Ok(CpuCycleResult {
                    memory_write: true,
                    address: Some(stack_addr),
                    data: Some(p),
                    bus_op: BusOperation::Write,
                    cycle_description: "Interrupt: push P",
                    ..Default::default()
                })
            }
            6 => {
                // Read interrupt vector low byte
                let vector_lo = self.bus.borrow().read_byte(vector)?;
                state.operand_lo = vector_lo;

                // Set interrupt disable flag
                self.registers.set_status(StatusFlag::InterruptDisable, true);

                Ok(CpuCycleResult {
                    memory_read: true,
                    address: Some(vector),
                    data: Some(vector_lo),
                    bus_op: BusOperation::Read,
                    cycle_description: "Interrupt: read vector low",
                    ..Default::default()
                })
            }
            7 => {
                // Read interrupt vector high byte
                let vector_hi = self.bus.borrow().read_byte(vector.wrapping_add(1))?;
                let new_pc = ((vector_hi as u16) << 8) | state.operand_lo as u16;

                self.registers.set_pc(new_pc);
                self.registers.is_pc_dirty = false;

                Ok(CpuCycleResult {
                    instruction_complete: true,
                    memory_read: true,
                    address: Some(vector.wrapping_add(1)),
                    data: Some(vector_hi),
                    bus_op: BusOperation::Read,
                    cycle_description: "Interrupt: read vector high, jump",
                    ..Default::default()
                })
            }
            _ => Err(CpuError::Unimplemented(format!("Interrupt cycle {} unexpected", cycle)))
        }
    }

    /// Check for pending interrupts using the LATCHED state from polling.
    /// This implements cycle-accurate interrupt timing where interrupts are
    /// polled during instruction execution, and the result is used after completion.
    /// Returns the number of cycles the interrupt will take (0 if no interrupt).
    #[deprecated(note = "Use check_interrupt_from_latch() + InterruptSequence state instead")]
    #[allow(dead_code)]
    fn check_and_setup_interrupt_from_latch(&mut self) -> Result<u32, CpuError> {
        // NMI has priority over IRQ
        if self.latched_nmi {
            // Clear pending_nmi since we're servicing it
            self.pending_nmi = false;
            self.nmi()?;
            Ok(Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES)
        } else if self.latched_irq {
            // Re-check the I flag - it could have been set during the instruction
            // (e.g., SEI instruction). The latched_irq indicates IRQ was pending
            // when polled, but the I flag state at completion determines if we service it.
            if !self.registers.get_status(StatusFlag::InterruptDisable) {
                self.irq()?;
                Ok(Cpu6502::INTERRUPT_HANDLER_NUM_CYCLES)
            } else {
                Ok(0)
            }
        } else {
            Ok(0)
        }
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
        let current_addr = STACK_BASE_ADDRESS + self.registers.sp as u16;

        // Dummy read from current SP location (6502 timing - internal SP increment)
        let _ = self.bus.borrow().read_byte(current_addr)?;

        // Now increment SP and read actual value
        let new_addr = Cpu6502::stack_wrapping_add(current_addr, 1);

        //debug!("CPU: sp (before pop): 0x{:02X}, popping at 0x{:04X}", self.registers.sp, new_addr);
        let value = self.bus.borrow().read_byte(new_addr)?;
        self.is_valid_stack_addr(new_addr)?;

        self.registers.sp = new_addr as u8;
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

    /// SBC operation core - performs subtraction with borrow on the given value
    /// Used by illegal opcodes that need to SBC without re-reading memory
    fn sbc_operation(&mut self, value: u8) {
        let carry = self.registers.get_status(StatusFlag::Carry) as u8;
        let a = self.registers.a;
        let (t0, overflow0) = a.overflowing_sub(value);
        let (t1, overflow1) = t0.overflowing_sub(1 - carry);

        self.registers.a = t1;

        self.registers.set_status(StatusFlag::Carry, !(overflow0 | overflow1));
        self.registers.set_status(StatusFlag::Zero, self.registers.a == 0);
        self.registers.set_status(StatusFlag::Negative, self.registers.a & 0x80 != 0);

        let overflow = ((a ^ self.registers.a) & 0x80 != 0) && ((a ^ value) & 0x80 != 0);
        self.registers.set_status(StatusFlag::Overflow, overflow);
    }

    /// ADC operation core - performs addition with carry on the given value
    /// Used by illegal opcodes that need to ADC without re-reading memory
    fn adc_operation(&mut self, value: u8) {
        let carry = self.registers.get_status(StatusFlag::Carry) as u8;
        let a = self.registers.a;
        let (t0, overflow0) = a.overflowing_add(value);
        let (t1, overflow1) = t0.overflowing_add(carry);

        self.registers.a = t1;

        self.registers.set_status(StatusFlag::Carry, overflow0 | overflow1);
        self.registers.set_status(StatusFlag::Zero, self.registers.a == 0);
        self.registers.set_status(StatusFlag::Negative, self.registers.a & 0x80 != 0);

        let overflow = ((a ^ self.registers.a) & 0x80 != 0) && !((a ^ value) & 0x80 != 0);
        self.registers.set_status(StatusFlag::Overflow, overflow);
    }

    /// DEPRECATED: Legacy method to write result to operand address.
    /// The cycle-stepped execution uses rmw_overwrite() with explicit cycle control.
    #[deprecated(note = "Legacy method - use rmw_overwrite() for cycle-accurate RMW")]
    #[allow(dead_code)]
    fn overwrite(&mut self, operand: &Operand, value: u8) -> Result<(), CpuError> {
        match operand {
            Operand::Address(addr) |
            Operand::AddressWithValue(addr, _) |
            Operand::AddressAndEffectiveAddress(_, addr, _) |
            Operand::AddressEffectiveWithValue(_, addr, _, _) => {
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

    /// RMW (Read-Modify-Write) overwrite - writes old value before new value
    /// This is required for cycle-accurate 6502 emulation as RMW instructions
    /// perform a dummy write of the old value before writing the new value.
    fn rmw_overwrite(&mut self, operand: &Operand, old_value: u8, new_value: u8) -> Result<(), CpuError> {
        match operand {
            // AddressWithValue and AddressEffectiveWithValue indicate cycle-accurate mode
            // Cycle code handles bus writes - just store the result for retrieval
            Operand::AddressWithValue(_, _) |
            Operand::AddressEffectiveWithValue(_, _, _, _) => {
                self.rmw_pending_value = Some(new_value);
                Ok(())
            },

            Operand::Address(addr) |
            Operand::AddressAndEffectiveAddress(_, addr, _) => {
                // 6502 RMW: write old value (dummy), then write new value
                self.bus.borrow_mut().write_byte(*addr, old_value)?;
                self.bus.borrow_mut().write_byte(*addr, new_value)?;
                Ok(())
            },

            Operand::Accumulator => {
                // Accumulator mode doesn't have this behavior
                self.registers.a = new_value;
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
            // Use pre-read value to avoid double-read (cycle-accurate mode)
            Operand::AddressWithValue(_, value) => {
                Ok(*value)
            },
            Operand::AddressAndEffectiveAddress(_, effective, _) => {
                let value = self.bus.borrow().read_byte(*effective)?;
                Ok(value)
            }
            // Use pre-read value to avoid double-read (cycle-accurate mode)
            Operand::AddressEffectiveWithValue(_, _, _, value) => {
                Ok(*value)
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
            Operand::AddressWithValue(addr, _) => {
                Ok(*addr)
            },
            Operand::AddressAndEffectiveAddress(_, effective, _) => {
                let value = *effective;
                Ok(value)
            },
            Operand::AddressEffectiveWithValue(_, effective, _, _) => {
                Ok(*effective)
            },
            _ => Err(CpuError::InvalidOperand(format!("{}", operand)))
        };

        value
    }

    /// Performs a dummy read for absolute indexed write operations.
    /// For absolute indexed modes (absx, absy), the 6502 always does a read before the write,
    /// even when the page boundary is not crossed. When page is crossed, the read
    /// is from the "wrong" address (base page + indexed low byte).
    ///
    /// Note: Zero-page indexed modes already have their dummy read in fetch_operand,
    /// so we only apply this for absolute indexed and indirect indexed Y modes.
    fn indexed_write_dummy_read(&self, addr_mode: AddressingMode, operand: &Operand) -> Result<(), CpuError> {
        // Apply to absolute indexed modes and indirect indexed Y
        let needs_dummy_read = matches!(addr_mode,
            AddressingMode::AbsoluteIndexedX | AddressingMode::AbsoluteIndexedY | AddressingMode::IndirectIndexedY);

        if needs_dummy_read {
            let (base, effective, page_crossed) = match operand {
                Operand::AddressAndEffectiveAddress(base, effective, page_crossed) => (*base, *effective, *page_crossed),
                Operand::AddressEffectiveWithValue(base, effective, page_crossed, _) => (*base, *effective, *page_crossed),
                _ => return Ok(()),
            };
            // Calculate the dummy read address:
            // - If page crossed: read from (base_high | effective_low) - the "wrong" address
            // - If not crossed: read from effective address (still a dummy read)
            let dummy_addr = if page_crossed {
                (base & 0xFF00) | (effective & 0x00FF)
            } else {
                effective
            };
            let _ = self.bus.borrow().read_byte(dummy_addr)?;
        }
        Ok(())
    }

    /// Performs a dummy read for indexed READ operations when page is crossed.
    /// For indexed read operations (absx, absy, (ind),y), when page boundary IS crossed,
    /// the 6502 first reads from the "wrong" address before reading from the correct address.
    /// This only applies when page is crossed (unlike writes which always do the dummy read).
    fn indexed_read_page_crossed_dummy(&self, addr_mode: AddressingMode, operand: &Operand) -> Result<(), CpuError> {
        // Apply to absolute indexed modes and indirect indexed Y for reads
        let is_indexed = matches!(addr_mode,
            AddressingMode::AbsoluteIndexedX | AddressingMode::AbsoluteIndexedY | AddressingMode::IndirectIndexedY);

        if is_indexed {
            let (base, effective, page_crossed) = match operand {
                Operand::AddressAndEffectiveAddress(base, effective, page_crossed) => (*base, *effective, *page_crossed),
                Operand::AddressEffectiveWithValue(base, effective, page_crossed, _) => (*base, *effective, *page_crossed),
                _ => return Ok(()),
            };
            // Only do dummy read when page is crossed
            if page_crossed {
                // Dummy read from "wrong" address (base_high | effective_low)
                let dummy_addr = (base & 0xFF00) | (effective & 0x00FF);
                let _ = self.bus.borrow().read_byte(dummy_addr)?;
            }
        }
        Ok(())
    }

    fn is_page_crossed(addr1: u16, addr2: u16) -> bool {
        let page1 = addr1 & 0xFF00;
        let page2 = addr2 & 0xFF00;

        page1 != page2
    }

    fn get_cycles_by_page_crossing_for_conditional_jump(&self, source: u16, destination: u16) -> u32 {
        if Cpu6502::is_page_crossed(source, destination) { 2 } else { 1 }
    }

    /// Perform cycle-accurate bus reads for taken branch instructions.
    /// When a branch is taken:
    /// - Cycle 3: Dummy read of next opcode byte (at PC after branch instruction)
    /// - Cycle 4 (if page crossed): Dummy read from wrong address
    fn branch_taken_dummy_reads(&self, source_pc: u16, dest_addr: u16) -> Result<(), CpuError> {
        // Cycle 3: Always read from the address after the branch instruction
        // (This would have been the next opcode if branch wasn't taken)
        let _ = self.bus.borrow().read_byte(source_pc)?;

        // Cycle 4: If page crossed, read from wrong address
        // The wrong address is: same page as source, but low byte from destination
        if Cpu6502::is_page_crossed(source_pc, dest_addr) {
            let wrong_addr = (source_pc & 0xFF00) | (dest_addr & 0x00FF);
            let _ = self.bus.borrow().read_byte(wrong_addr)?;
        }

        Ok(())
    }

    fn get_cycles_by_page_crossing_for_load(&self, operand: &Operand) -> u32 {
        match operand {
            Operand::AddressAndEffectiveAddress(_, _, page_crossed) |
            Operand::AddressEffectiveWithValue(_, _, page_crossed, _) => {
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
                // Dummy read of next byte (6502 always reads on every cycle)
                let _ = bus.borrow().read_byte(registers.safe_pc_add(1)?)?;
                Operand::None
            },

            AddressingMode::Accumulator => {
                // Dummy read of next byte (6502 always reads on every cycle)
                let _ = bus.borrow().read_byte(registers.safe_pc_add(1)?)?;
                Operand::Accumulator
            },

            AddressingMode::Immediate => {
                let pc = registers.safe_pc_add(1)?;

                Operand::Byte(bus.borrow().read_byte(pc)?)
            },

            AddressingMode::Absolute => {
                // JSR handles its own operand fetching with special cycle timing
                // (ADL read, stack operations, then ADH read)
                if instruction.opcode == OpCode::JSR {
                    Operand::None
                } else {
                    let pc = registers.safe_pc_add(1)?;
                    let addr = bus.borrow().read_word(pc)?;
                    Operand::Address(addr)
                }
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
                // Dummy read from base address before adding index (6502 timing)
                let _ = bus.borrow().read_byte(addr as u16)?;
                let effective_addr = addr.wrapping_add(registers.x) as u16;

                Operand::AddressAndEffectiveAddress(addr as u16, effective_addr, false)
            },

            AddressingMode::ZeroPageIndexedY => {
                let pc = registers.safe_pc_add(1)?;
                let addr = bus.borrow().read_byte(pc)?;
                // Dummy read from base address before adding index (6502 timing)
                let _ = bus.borrow().read_byte(addr as u16)?;
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
                // Dummy read from base address before adding X (6502 timing)
                let _ = bus.borrow().read_byte(addr as u16)?;
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

                // Store indirect_addr as base (needed for correct dummy read calculation in writes)
                Operand::AddressAndEffectiveAddress(indirect_addr, effective_addr, page_crossed)
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

    fn execute_instruction(&mut self, instruction: &Instruction, operand: &Operand) -> Result<u32, CpuError> {

        //debug!("CPU: executing instruction: opcode: {:?}, addressing mode: {:?}, operand: {}",
        //    instruction.opcode, instruction.addressing_mode, operand);

        (instruction.execute)(instruction, self, operand)
    }

    /// Executes the common interrupt sequence: push PC and status, set I flag.
    /// Returns the vector address to jump to (handles NMI hijacking for IRQ/BRK).
    /// - `return_pc`: The PC value to push onto stack (current PC for NMI/IRQ, PC+2 for BRK)
    /// - `brk`: True for BRK instruction (sets B flag in pushed status)
    /// - `default_vector`: The default interrupt vector (NMI_VECTOR, IRQ_VECTOR, or BRK_VECTOR)
    fn interrupt_preamble(&mut self, return_pc: u16, brk: bool, default_vector: u16) -> Result<u16, CpuError> {
        self.push_stack((return_pc >> 8) as u8)?;
        self.push_stack((return_pc & 0xFF) as u8)?;

        let status = if brk {
            self.registers.p | StatusFlag::BreakCommand.bits() | StatusFlag::Unused.bits()
        } else {
            (self.registers.p & !StatusFlag::BreakCommand.bits()) | StatusFlag::Unused.bits()
        };

        self.push_stack(status)?;
        self.registers.set_status(StatusFlag::InterruptDisable, true);

        // NMI can hijack IRQ/BRK if it arrives during the interrupt sequence
        let vector = if default_vector != NMI_VECTOR && self.pending_nmi {
            self.pending_nmi = false;
            NMI_VECTOR
        } else {
            default_vector
        };

        Ok(self.bus.borrow().read_word(vector)?)
    }

    fn nmi(&mut self) -> Result<(), CpuError> {
        self.pending_nmi = false;
        self.registers.pc = self.interrupt_preamble(self.registers.pc, false, NMI_VECTOR)?;
        Ok(())
    }

    fn irq(&mut self) -> Result<(), CpuError> {
        self.registers.pc = self.interrupt_preamble(self.registers.pc, false, IRQ_VECTOR)?;
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

    #[cfg(test)]
    pub fn get_a(&self) -> u8 {
        self.registers.a
    }

    #[cfg(test)]
    pub fn set_a(&mut self, value: u8) {
        self.registers.a = value;
    }

    #[cfg(test)]
    pub fn get_x(&self) -> u8 {
        self.registers.x
    }

    #[cfg(test)]
    pub fn set_x(&mut self, value: u8) {
        self.registers.x = value;
    }

    #[cfg(test)]
    pub fn get_y(&self) -> u8 {
        self.registers.y
    }

    #[cfg(test)]
    pub fn set_y(&mut self, value: u8) {
        self.registers.y = value;
    }

    #[cfg(test)]
    pub fn get_sp(&self) -> u8 {
        self.registers.sp
    }

    #[cfg(test)]
    pub fn set_sp(&mut self, value: u8) {
        self.registers.sp = value;
    }

    #[cfg(test)]
    pub fn get_pc(&self) -> u16 {
        self.registers.pc
    }

    #[cfg(test)]
    pub fn set_pc_for_test(&mut self, value: u16) {
        self.registers.pc = value;
    }

    #[cfg(test)]
    pub fn get_status(&self) -> u8 {
        self.registers.p
    }

    #[cfg(test)]
    pub fn set_status_for_test(&mut self, value: u8) {
        self.registers.p = value;
    }

    #[cfg(test)]
    pub fn get_carry(&self) -> bool {
        self.registers.get_status(StatusFlag::Carry)
    }

    #[cfg(test)]
    pub fn set_carry(&mut self, value: bool) {
        self.registers.set_status(StatusFlag::Carry, value);
    }

    #[cfg(test)]
    pub fn get_zero(&self) -> bool {
        self.registers.get_status(StatusFlag::Zero)
    }

    #[cfg(test)]
    pub fn get_interrupt_disable(&self) -> bool {
        self.registers.get_status(StatusFlag::InterruptDisable)
    }

    #[cfg(test)]
    pub fn set_interrupt_disable(&mut self, value: bool) {
        self.registers.set_status(StatusFlag::InterruptDisable, value);
    }

    #[cfg(test)]
    pub fn get_decimal(&self) -> bool {
        self.registers.get_status(StatusFlag::DecimalMode)
    }

    #[cfg(test)]
    pub fn get_overflow(&self) -> bool {
        self.registers.get_status(StatusFlag::Overflow)
    }

    #[cfg(test)]
    pub fn set_overflow(&mut self, value: bool) {
        self.registers.set_status(StatusFlag::Overflow, value);
    }

    #[cfg(test)]
    pub fn get_negative(&self) -> bool {
        self.registers.get_status(StatusFlag::Negative)
    }

    #[cfg(test)]
    pub fn clear_pending_nmi(&mut self) {
        self.pending_nmi = false;
        self.nmi_line_low = false;
    }

    #[cfg(test)]
    pub fn set_state_for_test(&mut self, pc: u16, sp: u8, a: u8, x: u8, y: u8, p: u8) {
        self.registers.pc = pc;
        self.registers.sp = sp;
        self.registers.a = a;
        self.registers.x = x;
        self.registers.y = y;
        self.registers.p = p;
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
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
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
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = cpu.registers.a & value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn asl_shift_left_one_bit(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = cpu.shift_left_and_update_carry_flags(value);

        cpu.update_flags_zero_negative(result);
        cpu.rmw_overwrite(operand, value, result)?;

        Ok(0)
    }

    fn bcc_branch_on_carry_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Carry) {
            let addr = cpu.get_operand_word_value(operand)?;
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn bit_test_bits_in_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Use get_operand_byte_value to use pre-read value in cycle-accurate mode
        let value = cpu.get_operand_byte_value(operand)?;
        let result = cpu.registers.a & value;

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Overflow, value & 0x40 != 0);

        Ok(0)
    }

    fn bmi_branch_on_result_minus(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if cpu.registers.get_status(StatusFlag::Negative) {
            let addr = cpu.get_operand_word_value(operand)?;
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

            //debug!("CPU: branching to address {:04X}", addr);
            cpu.registers.set_pc(addr);

            Ok(cycles)
        } else {
            Ok(0)
        }
    }

    fn brk_force_break(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let return_pc = cpu.registers.safe_pc_add(2)?;
        let addr = cpu.interrupt_preamble(return_pc, true, BRK_VECTOR)?;
        cpu.registers.set_pc(addr);
        Ok(0)
    }

    fn bvc_branch_on_overflow_clear(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        if !cpu.registers.get_status(StatusFlag::Overflow) {
            let addr = cpu.get_operand_word_value(operand)?;
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
            let source_pc = cpu.registers.safe_pc_add(self.bytes as i16)?;
            let cycles = cpu.get_cycles_by_page_crossing_for_conditional_jump(source_pc, addr);

            // Cycle-accurate dummy reads for taken branch
            cpu.branch_taken_dummy_reads(source_pc, addr)?;

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
        cpu.pending_i_flag = Some(false);
        Ok(0)
    }

    fn clv_clear_overflow_flag(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        cpu.registers.set_status(StatusFlag::Overflow, false);
        Ok(0)
    }

    fn cmp_compare_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
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
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_sub(1);

        cpu.update_flags_zero_negative(result);
        cpu.rmw_overwrite(operand, value, result)?;

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
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = cpu.registers.a ^ value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn inc_increment_memory_by_one(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_add(1);

        cpu.update_flags_zero_negative(result);
        cpu.rmw_overwrite(operand, value, result)?;

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


    fn jsr_jump_to_new_location_saving_return_address(&self, cpu: &mut Cpu6502, _operand: &Operand) -> Result<u32, CpuError> {
        // JSR has a unique cycle sequence that differs from normal absolute addressing:
        // Cycle 1: Fetch opcode (already done)
        // Cycle 2: Fetch ADL (low byte of target address)
        // Cycle 3: Dummy read from stack location (internal operation)
        // Cycle 4: Push PCH
        // Cycle 5: Push PCL
        // Cycle 6: Fetch ADH (high byte of target address)
        //
        // Note: The operand was pre-fetched with both bytes read together,
        // so we ignore it and re-read the bytes in the correct order.
        // The duplicate reads don't affect correctness, just cycle tracing.

        let pc_plus_1 = cpu.registers.safe_pc_add(1)?;
        let pc_plus_2 = cpu.registers.safe_pc_add(2)?;

        // Cycle 2: Already done in fetch_operand, re-read ADL for proper ordering
        // (This read is "wasted" but needed for cycle-accurate bus tracing)
        let adl = cpu.bus.borrow().read_byte(pc_plus_1)?;

        // Cycle 3: Dummy read from current stack location
        let stack_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let _ = cpu.bus.borrow().read_byte(stack_addr)?;

        // Cycle 4: Push PCH (return address high byte)
        cpu.push_stack((pc_plus_2 >> 8) as u8)?;

        // Cycle 5: Push PCL (return address low byte)
        cpu.push_stack(pc_plus_2 as u8)?;

        // Cycle 6: Fetch ADH (high byte of target)
        let adh = cpu.bus.borrow().read_byte(pc_plus_2)?;

        // Set PC to target address
        let target_addr = (adh as u16) << 8 | adl as u16;
        cpu.registers.set_pc(target_addr);

        Ok(0)
    }


    fn lda_load_accumulator_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.a = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);

        Ok(cycles)
    }

    fn ldx_load_index_x_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.x = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.x == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.x & 0x80 != 0);

        Ok(cycles)
    }

    fn ldy_load_index_y_with_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        cpu.registers.y = value;

        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.y == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.y & 0x80 != 0);

        Ok(cycles)
    }

    fn lsr_shift_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = cpu.shift_right_and_update_carry_flags(value);

        cpu.update_flags_zero_negative(result);
        cpu.rmw_overwrite(operand, value, result)?;

        Ok(0)
    }

    fn nop_no_operation(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For non-implicit modes, NOP reads from the address (and discards result)
        // Indexed modes need dummy read on page crossing
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;

        // Read from operand address (discarded, but needed for bus cycle accuracy)
        match operand {
            Operand::Address(_) | Operand::AddressAndEffectiveAddress(_, _, _) => {
                let _ = cpu.get_operand_byte_value(operand)?;
            },
            _ => {}
        }

        let cycle = cpu.get_cycles_by_page_crossing_for_load(operand);
        Ok(cycle)
    }

    fn ora_or_memory_with_accumulator(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read from wrong address when page crossed (indexed modes)
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
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

    fn plp_pull_processor_status_from_stack(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        let status = cpu.pop_stack()?;

        let new_i_flag = (status & StatusFlag::InterruptDisable.bits()) != 0;
        let old_i_flag = cpu.registers.get_status(StatusFlag::InterruptDisable);

        cpu.registers.p = (cpu.registers.p & 0x34) | (status & !0x34);

        if new_i_flag != old_i_flag {
            cpu.pending_i_flag = Some(new_i_flag);
        }

        Ok(0)
    }

    fn rol_rotate_one_bit_left(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x01 } else { 0 };
        let result = (value << 1) | carry_in;

        cpu.rmw_overwrite(operand, value, result)?;

        cpu.registers.set_status(StatusFlag::Carry, value & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(0)
    }

    fn ror_rotate_one_bit_right(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // For absolute indexed RMW, dummy read from wrong/effective address before value read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
        let result = (value >> 1) | carry_in;

        cpu.rmw_overwrite(operand, value, result)?;

        cpu.registers.set_status(StatusFlag::Carry, value & 0x01 != 0);
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(0)
    }


    fn rti_return_from_interrupt(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        // RTI cycle sequence:
        // Cycle 1: Fetch opcode (already done)
        // Cycle 2: Dummy read of next byte (done by fetch_operand for Implicit mode)
        // Cycle 3: Dummy read from stack location (SP)
        // Cycle 4: Increment SP, read P (status) from stack
        // Cycle 5: Increment SP, read PCL from stack
        // Cycle 6: Increment SP, read PCH from stack

        // Cycle 3: Dummy read from current stack location
        let stack_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let _ = cpu.bus.borrow().read_byte(stack_addr)?;

        // Cycle 4: Increment SP and read status
        cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
        let status_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let status = cpu.bus.borrow().read_byte(status_addr)?;

        // Cycle 5: Increment SP and read PCL
        cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
        let pcl_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let pcl = cpu.bus.borrow().read_byte(pcl_addr)?;

        // Cycle 6: Increment SP and read PCH
        cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
        let pch_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let pch = cpu.bus.borrow().read_byte(pch_addr)?;

        // Handle I flag changes with delay
        let new_i_flag = (status & StatusFlag::InterruptDisable.bits()) != 0;
        let old_i_flag = cpu.registers.get_status(StatusFlag::InterruptDisable);

        // Restore P: ignore bit 4 (B flag, doesn't exist in CPU), keep bit 5 always set
        // Mask: clear bit 4, set bit 5
        cpu.registers.p = (status & 0xEF) | 0x20;

        if new_i_flag != old_i_flag {
            cpu.pending_i_flag = Some(new_i_flag);
        }

        // Set PC to return address
        let return_addr = (pch as u16) << 8 | pcl as u16;
        cpu.registers.set_pc(return_addr);

        Ok(0)
    }

    fn rts_return_from_subroutine(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        // RTS cycle sequence:
        // Cycle 1: Fetch opcode (already done)
        // Cycle 2: Dummy read of next byte (done by fetch_operand for Implicit mode)
        // Cycle 3: Dummy read from stack location (SP)
        // Cycle 4: Increment SP, read PCL from stack
        // Cycle 5: Increment SP, read PCH from stack
        // Cycle 6: Dummy read from new PC (to increment returned address)

        // Cycle 3: Dummy read from current stack location
        let stack_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let _ = cpu.bus.borrow().read_byte(stack_addr)?;

        // Cycle 4: Increment SP and read PCL
        cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
        let pcl_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let pcl = cpu.bus.borrow().read_byte(pcl_addr)?;

        // Cycle 5: Increment SP and read PCH
        cpu.registers.sp = cpu.registers.sp.wrapping_add(1);
        let pch_addr = STACK_BASE_ADDRESS + cpu.registers.sp as u16;
        let pch = cpu.bus.borrow().read_byte(pch_addr)?;

        // Construct return address (points to last byte of JSR instruction)
        let return_addr = (pch as u16) << 8 | pcl as u16;

        // Cycle 6: Dummy read from return address (while incrementing PC)
        let _ = cpu.bus.borrow().read_byte(return_addr)?;

        // Set PC to return address + 1 (address after JSR instruction)
        cpu.registers.set_pc(return_addr.wrapping_add(1));

        Ok(0)
    }

    fn sbc_subtract_memory_from_accumulator_with_borrow(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
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
        cpu.pending_i_flag = Some(true);
        Ok(0)
    }

    fn sta_store_accumulator_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let addr = cpu.get_operand_word_value(operand)?;

        cpu.bus.borrow_mut().write_byte(addr, cpu.registers.a)?;

        Ok(0)
    }

    fn stx_store_index_x_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let addr = cpu.get_operand_word_value(operand)?;

        cpu.bus.borrow_mut().write_byte(addr, cpu.registers.x)?;

        Ok(0)
    }

    fn sty_store_index_y_in_memory(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
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
        // ANE/XAA: A = (A | magic) & X & operand
        // Magic constant is CPU-specific. Using 0xEE based on common behavior
        let value = cpu.get_operand_byte_value(operand)?;
        let result = (cpu.registers.a | 0xEE) & cpu.registers.x & value;

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
        // DCP = DEC + CMP (without extra read for CMP)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_sub(1);
        cpu.rmw_overwrite(operand, value, result)?;

        // CMP with the decremented result (no additional bus read)
        let diff = cpu.registers.a.wrapping_sub(result);
        cpu.registers.set_status(StatusFlag::Zero, diff == 0);
        cpu.registers.set_status(StatusFlag::Negative, diff & 0x80 != 0);
        cpu.registers.set_status(StatusFlag::Carry, cpu.registers.a >= result);
        Ok(0)
    }

    fn isb_inc_plus_sbc(&self, _: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        Err(CpuError::Unimplemented(format!("{:?}", self.opcode)))
    }

    fn isc_inc_oper_plus_sbc_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // ISC = INC + SBC (without extra read for SBC)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let result = value.wrapping_add(1);
        cpu.rmw_overwrite(operand, value, result)?;

        // SBC with the incremented result (no additional bus read)
        cpu.sbc_operation(result);
        Ok(0)
    }

    fn jam_freeze_the_cpu(&self, cpu: &mut Cpu6502, _: &Operand) -> Result<u32, CpuError> {
        // JAM/KIL/HLT freezes the CPU but does specific bus activity:
        // Cycle 1: Fetch opcode (already done)
        // Cycle 2: Fetch operand (dummy read, done by Implicit mode)
        // Cycles 3-11: Read from interrupt vector area in specific pattern
        //   3: Read $FFFF
        //   4: Read $FFFE
        //   5: Read $FFFE
        //   6-11: Read $FFFF (6 times)

        // Read $FFFF
        let _ = cpu.bus.borrow().read_byte(0xFFFF)?;
        // Read $FFFE
        let _ = cpu.bus.borrow().read_byte(0xFFFE)?;
        // Read $FFFE again
        let _ = cpu.bus.borrow().read_byte(0xFFFE)?;
        // Read $FFFF 6 more times
        for _ in 0..6 {
            let _ = cpu.bus.borrow().read_byte(0xFFFF)?;
        }

        // PC is already advanced by instruction.bytes (1) by step_instruction
        Ok(0)
    }

    fn las_lda_tsx_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // LAS (0xBB): A = X = S = (memory & SP)
        // Uses absolute,Y addressing
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;

        // AND memory with stack pointer, store in A, X, and S
        let result = value & cpu.registers.sp;
        cpu.registers.a = result;
        cpu.registers.x = result;
        cpu.registers.sp = result;

        // Set flags based on result
        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(cpu.get_cycles_by_page_crossing_for_load(operand))
    }

    fn lax_lda_oper_plus_ldx_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // LAX = LDA + LDX (but only one memory read)
        cpu.indexed_read_page_crossed_dummy(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let cycles = cpu.get_cycles_by_page_crossing_for_load(operand);

        // Load both A and X with the same value
        cpu.registers.a = value;
        cpu.registers.x = value;

        cpu.registers.set_status(StatusFlag::Zero, value == 0);
        cpu.registers.set_status(StatusFlag::Negative, value & 0x80 != 0);

        Ok(cycles)
    }

    fn lxa_store_and_oper_in_a_and_x(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // LXA/LAX imm/ATX: A = X = (A | magic) & operand
        // Uses same magic constant as ANE (0xEE)
        let value = cpu.get_operand_byte_value(operand)?;
        let result = (cpu.registers.a | 0xEE) & value;

        cpu.registers.a = result;
        cpu.registers.x = result;

        cpu.registers.set_status(StatusFlag::Zero, result == 0);
        cpu.registers.set_status(StatusFlag::Negative, result & 0x80 != 0);

        Ok(0)
    }

    fn rla_rol_oper_plus_and_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // RLA = ROL + AND (without extra read for AND)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x01 } else { 0 };
        let result = (value << 1) | carry_in;
        cpu.registers.set_status(StatusFlag::Carry, value & 0x80 != 0);
        cpu.rmw_overwrite(operand, value, result)?;

        // AND with the rotated result (no additional bus read)
        cpu.registers.a = cpu.registers.a & result;
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);
        Ok(0)
    }

    fn rra_ror_oper_plus_adc_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // RRA = ROR + ADC (without extra read for ADC)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let carry_in = if cpu.registers.get_status(StatusFlag::Carry) { 0x80 } else { 0 };
        let result = (value >> 1) | carry_in;
        cpu.registers.set_status(StatusFlag::Carry, value & 0x01 != 0);
        cpu.rmw_overwrite(operand, value, result)?;

        // ADC with the rotated result (no additional bus read)
        cpu.adc_operation(result);
        Ok(0)
    }

    fn sax_axs_aax(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
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
     *   suitable target address based on what kind of values you want to store.
     *  - https://hitmen.c02.at/files/docs/c64/NoMoreSecrets-NMOS6510UnintendedOpcodes-20162412.pdf
     ***/
    fn sha_stores_a_and_x_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;

        // SHA stores A & X & (H+1) where H is high byte of BASE address (not effective)
        let (base_addr, effective_addr, page_crossed) = match operand {
            Operand::AddressAndEffectiveAddress(base, effective, page_crossed) => (*base, *effective, *page_crossed),
            Operand::AddressEffectiveWithValue(base, effective, page_crossed, _) => (*base, *effective, *page_crossed),
            Operand::Address(addr) => (*addr, *addr, false),
            _ => { unreachable!() }
        };

        // H is the high byte of the base address
        let h = (base_addr >> 8) as u8;
        let value = cpu.registers.a & cpu.registers.x & h.wrapping_add(1);

        if page_crossed {
            // When page crossed, address gets corrupted: low byte from effective, high byte from value
            let target = (effective_addr & 0x00FF) | ((value as u16) << 8);
            cpu.bus.borrow_mut().write_byte(target, value)?;
        } else {
            cpu.bus.borrow_mut().write_byte(effective_addr, value)?;
        };

        Ok(0)
    }

    fn shx_stores_x_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;

        // SHX stores X & (H+1) where H is high byte of BASE address (not effective)
        let (base_addr, effective_addr, page_crossed) = match operand {
            Operand::AddressAndEffectiveAddress(base, effective, page_crossed) => (*base, *effective, *page_crossed),
            Operand::AddressEffectiveWithValue(base, effective, page_crossed, _) => (*base, *effective, *page_crossed),
            Operand::Address(addr) => (*addr, *addr, false),
            _ => { unreachable!() }
        };

        // H is the high byte of the base address
        let h = (base_addr >> 8) as u8;
        let value = cpu.registers.x & h.wrapping_add(1);

        if page_crossed {
            // When page crossed, address gets corrupted: low byte from effective, high byte from value
            let target = (effective_addr & 0x00FF) | ((value as u16) << 8);
            cpu.bus.borrow_mut().write_byte(target, value)?;
        } else {
            cpu.bus.borrow_mut().write_byte(effective_addr, value)?;
        };

        Ok(0)
    }

    fn shy_stores_y_and_at_addr(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // Dummy read for absolute indexed modes (6502 always reads before writing on indexed stores)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;

        // SHY stores Y & (H+1) where H is high byte of BASE address (not effective)
        let (base_addr, effective_addr, page_crossed) = match operand {
            Operand::AddressAndEffectiveAddress(base, effective, page_crossed) => (*base, *effective, *page_crossed),
            Operand::AddressEffectiveWithValue(base, effective, page_crossed, _) => (*base, *effective, *page_crossed),
            Operand::Address(addr) => (*addr, *addr, false),
            _ => { unreachable!() }
        };

        // H is the high byte of the base address
        let h = (base_addr >> 8) as u8;
        let value = cpu.registers.y & h.wrapping_add(1);

        if page_crossed {
            // When page crossed, address gets corrupted: low byte from effective, high byte from value
            let target = (effective_addr & 0x00FF) | ((value as u16) << 8);
            cpu.bus.borrow_mut().write_byte(target, value)?;
        } else {
            cpu.bus.borrow_mut().write_byte(effective_addr, value)?;
        };

        Ok(0)
    }

    fn slo_asl_oper_plus_ora_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // SLO = ASL + ORA (without extra read for ORA)
        // Indexed modes need dummy read
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let shifted = cpu.shift_left_and_update_carry_flags(value);
        cpu.rmw_overwrite(operand, value, shifted)?;

        // ORA with the shifted result (no additional bus read)
        cpu.registers.a = cpu.registers.a | shifted;
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);
        Ok(0)
    }

    fn sre_lsr_oper_plus_eor_oper(&self, cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
        // SRE = LSR + EOR (without extra read for EOR)
        cpu.indexed_write_dummy_read(self.addressing_mode, operand)?;
        let value = cpu.get_operand_byte_value(operand)?;
        let shifted = cpu.shift_right_and_update_carry_flags(value);
        cpu.rmw_overwrite(operand, value, shifted)?;

        // EOR with the shifted result (no additional bus read)
        cpu.registers.a = cpu.registers.a ^ shifted;
        cpu.registers.set_status(StatusFlag::Zero, cpu.registers.a == 0);
        cpu.registers.set_status(StatusFlag::Negative, cpu.registers.a & 0x80 != 0);
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