// Authorship: Human 0% | Claude 100%
//! MMC3 (Mapper 4) cartridge implementation.
//!
//! MMC3 is one of the most common NES mappers, used by games like Super Mario Bros. 3,
//! Mega Man 3-6, and many others.
//!
//! ## PRG-ROM Banking
//! - Four 8KB slots at $8000-$FFFF
//! - Two modes controlled by prg_mode bit:
//!   - Mode 0: R6 at $8000, R7 at $A000, fixed(-2) at $C000, fixed(-1) at $E000
//!   - Mode 1: fixed(-2) at $8000, R7 at $A000, R6 at $C000, fixed(-1) at $E000
//!
//! ## CHR Banking
//! - Eight 1KB slots at $0000-$1FFF
//! - Two modes controlled by chr_mode bit:
//!   - Mode 0: R0(2KB), R1(2KB), R2-R5(1KB each)
//!   - Mode 1: R2-R5(1KB), R0(2KB), R1(2KB)
//! - R0/R1 are forced to even alignment (bit 0 ignored)
//!
//! ## IRQ Counter
//! - Clocked by PPU A12 rising edges with 8 PPU cycle filter
//! - Counter reload on zero or when reload flag set
//! - IRQ fired when counter reaches 0 and IRQ enabled
//!
//! ## PRG-RAM
//! - 8KB at $6000-$7FFF with enable/write-protect control

use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use log::{debug, trace};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::{self, Cartridge, CartridgeError, CartridgeType, CPU_ADDRESS_SPACE, PPU_ADDRESS_SPACE};
use crate::ines_loader::{FromINes, INesRomHeader, Region};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::memory_ciram::PpuNameTableMirroring;

/// MMC3 uses 8KB PRG-ROM banks
const MMC3_PRG_BANK_SIZE: usize = 8 * 1024;

/// MMC3 uses 1KB CHR banks
const MMC3_CHR_BANK_SIZE: usize = 1024;

/// PRG-RAM address space ($6000-$7FFF)
const PRG_RAM_ADDRESS_SPACE: (u16, u16) = (0x6000, 0x7FFF);

/// PRG-RAM bank size (8KB)
const PRG_RAM_BANK_SIZE: usize = 8 * 1024;

/// A12 filter threshold in PPU cycles (minimum low time before counting rising edge)
const A12_FILTER_CYCLES: u32 = 8;

const MAPPER_NAME: &str = "MMC3";

/// Shared state between PRG and CHR memory
#[derive(Debug)]
struct Mmc3SharedState {
    /// CHR memory banks (1KB each) - can be ROM or RAM
    chr_banks: Vec<MemoryBank>,
    /// Number of 1KB CHR banks
    num_chr_banks: usize,
    /// True if CHR is ROM (writes ignored), false if RAM
    chr_is_rom: bool,

    /// Bank register index (0-7), set by $8000 bits 0-2
    bank_register_index: usize,
    /// PRG bank mode (0 or 1), set by $8000 bit 6
    prg_mode: u8,
    /// CHR bank mode (0 or 1), set by $8000 bit 7
    chr_mode: u8,
    /// Bank registers R0-R7
    bank_registers: [u8; 8],

    /// IRQ counter current value
    irq_counter: u8,
    /// IRQ latch value (loaded into counter)
    irq_latch: u8,
    /// IRQ reload flag (counter reloaded on next A12 edge)
    irq_reload: bool,
    /// IRQ enabled flag
    irq_enabled: bool,
    /// IRQ pending (line asserted)
    irq_pending: Cell<bool>,

    /// A12 state tracking for edge detection
    a12_low_cycles: u32,
    last_a12_state: bool,
}

impl Mmc3SharedState {
    fn new(chr_banks: Vec<MemoryBank>, num_chr_banks: usize, chr_is_rom: bool) -> Self {
        Self {
            chr_banks,
            num_chr_banks,
            chr_is_rom,
            bank_register_index: 0,
            prg_mode: 0,
            chr_mode: 0,
            bank_registers: [0; 8],
            irq_counter: 0,
            irq_latch: 0,
            irq_reload: false,
            irq_enabled: false,
            irq_pending: Cell::new(false),
            a12_low_cycles: 0,
            last_a12_state: false,
        }
    }

    /// Get the CHR bank index for a given PPU address ($0000-$1FFF)
    fn get_chr_bank_for_address(&self, addr: u16) -> usize {
        let slot = (addr / 0x400) as usize; // 0-7 (1KB slots)

        let bank_index = match (self.chr_mode, slot) {
            // Mode 0: R0(2KB), R1(2KB), R2, R3, R4, R5
            (0, 0) => (self.bank_registers[0] & 0xFE) as usize,     // R0 even
            (0, 1) => (self.bank_registers[0] & 0xFE) as usize + 1, // R0 odd
            (0, 2) => (self.bank_registers[1] & 0xFE) as usize,     // R1 even
            (0, 3) => (self.bank_registers[1] & 0xFE) as usize + 1, // R1 odd
            (0, 4) => self.bank_registers[2] as usize,
            (0, 5) => self.bank_registers[3] as usize,
            (0, 6) => self.bank_registers[4] as usize,
            (0, 7) => self.bank_registers[5] as usize,
            // Mode 1: R2, R3, R4, R5, R0(2KB), R1(2KB)
            (1, 0) => self.bank_registers[2] as usize,
            (1, 1) => self.bank_registers[3] as usize,
            (1, 2) => self.bank_registers[4] as usize,
            (1, 3) => self.bank_registers[5] as usize,
            (1, 4) => (self.bank_registers[0] & 0xFE) as usize,     // R0 even
            (1, 5) => (self.bank_registers[0] & 0xFE) as usize + 1, // R0 odd
            (1, 6) => (self.bank_registers[1] & 0xFE) as usize,     // R1 even
            (1, 7) => (self.bank_registers[1] & 0xFE) as usize + 1, // R1 odd
            _ => 0,
        };

        // Wrap bank index within available banks
        if self.num_chr_banks > 0 {
            bank_index % self.num_chr_banks
        } else {
            0
        }
    }

    /// Notify the mapper of a PPU address access (for A12 edge detection)
    fn notify_ppu_address(&mut self, addr: u16) {
        let a12 = (addr & 0x1000) != 0;

        if a12 {
            // A12 is high
            if !self.last_a12_state && self.a12_low_cycles >= A12_FILTER_CYCLES {
                // Rising edge detected with sufficient low time - clock the counter
                self.clock_irq_counter();
            }
            self.a12_low_cycles = 0;
        } else {
            // A12 is low - increment low cycle counter
            self.a12_low_cycles = self.a12_low_cycles.saturating_add(1);
        }

        self.last_a12_state = a12;
    }

    /// Clock the IRQ counter (called on qualified A12 rising edge)
    fn clock_irq_counter(&mut self) {
        if self.irq_counter == 0 || self.irq_reload {
            self.irq_counter = self.irq_latch;
            self.irq_reload = false;
        } else {
            self.irq_counter = self.irq_counter.saturating_sub(1);
        }

        // Fire IRQ when counter reaches 0 and IRQ is enabled
        if self.irq_counter == 0 && self.irq_enabled {
            self.irq_pending.set(true);
            trace!("MMC3: IRQ fired!");
        }
    }
}

#[derive(Debug)]
pub struct Mmc3Cartridge {
    /// PRG-ROM banks (8KB each)
    prg_rom_banks: Vec<MemoryBank>,
    /// Number of 8KB PRG-ROM banks
    num_prg_banks: usize,

    /// Shared state with CHR memory
    shared: Rc<RefCell<Mmc3SharedState>>,

    /// Optional 8KB PRG-RAM at $6000-$7FFF
    prg_ram: Option<Rc<RefCell<MemoryBank>>>,
    /// PRG-RAM enabled (bit 7 of $A001)
    prg_ram_enabled: bool,
    /// PRG-RAM write protected (bit 6 of $A001)
    prg_ram_write_protect: bool,

    /// Nametable mirroring
    mirroring: Rc<RefCell<PpuNameTableMirroring>>,
    /// Whether mirroring is fixed by board (ignores $A000 writes)
    mirroring_fixed: bool,

    /// CHR memory wrapper for get_chr_rom()
    chr_memory: Rc<RefCell<Mmc3ChrMemory>>,

    /// Bus device type identifier
    device_type: BusDeviceType,
    /// Console region (NTSC/PAL)
    region: Region,
}

impl Mmc3Cartridge {
    /// Create a new MMC3 cartridge.
    pub fn new(
        mut data: BufReader<File>,
        prg_rom_offset: u64,
        prg_rom_size: usize,
        chr_rom_size: usize,
        chr_ram_size: usize,
        has_prg_ram: bool,
        mirroring: PpuNameTableMirroring,
        mirroring_fixed: bool,
        region: Region,
    ) -> Result<Self, CartridgeError> {
        // Load PRG-ROM banks (8KB each)
        let prg_rom_banks = cartridge::create_split_rom_memory(
            &mut data,
            prg_rom_offset,
            prg_rom_size,
            MMC3_PRG_BANK_SIZE,
            CPU_ADDRESS_SPACE,
        )?;
        let num_prg_banks = prg_rom_banks.len();
        debug!("MMC3: Loaded {} 8KB PRG-ROM banks", num_prg_banks);

        // Determine CHR type and size
        let (chr_size, chr_is_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);

        // Load or create CHR banks (1KB each)
        let chr_banks = if chr_is_rom {
            let chr_offset = prg_rom_offset + prg_rom_size as u64;
            cartridge::create_split_rom_memory(
                &mut data,
                chr_offset,
                chr_size,
                MMC3_CHR_BANK_SIZE,
                PPU_ADDRESS_SPACE,
            )?
        } else {
            cartridge::create_split_ram_memory(
                chr_size,
                MMC3_CHR_BANK_SIZE,
                PPU_ADDRESS_SPACE,
            )?
        };
        let num_chr_banks = chr_banks.len();
        debug!("MMC3: Created {} 1KB CHR-{} banks", num_chr_banks, if chr_is_rom { "ROM" } else { "RAM" });

        // Create PRG-RAM if specified
        let prg_ram = if has_prg_ram {
            let ram = MemoryBank::new(PRG_RAM_BANK_SIZE, PRG_RAM_ADDRESS_SPACE);
            debug!("MMC3: Created 8KB PRG-RAM at $6000-$7FFF");
            Some(Rc::new(RefCell::new(ram)))
        } else {
            None
        };

        let mirroring_rc = Rc::new(RefCell::new(mirroring));

        // Create shared state
        let shared = Rc::new(RefCell::new(Mmc3SharedState::new(chr_banks, num_chr_banks, chr_is_rom)));

        // Create CHR memory wrapper
        let chr_memory = Rc::new(RefCell::new(Mmc3ChrMemory::new(shared.clone())));

        Ok(Self {
            prg_rom_banks,
            num_prg_banks,
            shared,
            prg_ram,
            prg_ram_enabled: false,
            prg_ram_write_protect: false,
            mirroring: mirroring_rc,
            mirroring_fixed,
            chr_memory,
            device_type: BusDeviceType::CARTRIDGE(CartridgeType::MMC3),
            region,
        })
    }

    fn build(
        file: File,
        prg_rom_offset: u64,
        prg_rom_size: usize,
        chr_rom_size: usize,
        chr_ram_size: usize,
        has_prg_ram: bool,
        mirroring: PpuNameTableMirroring,
        mirroring_fixed: bool,
        region: Region,
    ) -> Result<Mmc3Cartridge, LoaderError> {
        debug!("creating MMC3 cartridge");

        let reader = BufReader::new(file);
        let cartridge = Mmc3Cartridge::new(
            reader,
            prg_rom_offset,
            prg_rom_size,
            chr_rom_size,
            chr_ram_size,
            has_prg_ram,
            mirroring,
            mirroring_fixed,
            region,
        )?;

        Ok(cartridge)
    }

    /// Get the PRG-ROM bank index for a given CPU address ($8000-$FFFF)
    fn get_prg_bank_for_address(&self, addr: u16) -> usize {
        let slot = ((addr - 0x8000) / 0x2000) as usize; // 0-3
        let shared = self.shared.borrow();

        let bank_index = match (shared.prg_mode, slot) {
            // Mode 0: R6, R7, fixed(-2), fixed(-1)
            (0, 0) => shared.bank_registers[6] as usize,
            (0, 1) => shared.bank_registers[7] as usize,
            (0, 2) => self.num_prg_banks.saturating_sub(2),
            (0, 3) => self.num_prg_banks.saturating_sub(1),
            // Mode 1: fixed(-2), R7, R6, fixed(-1)
            (1, 0) => self.num_prg_banks.saturating_sub(2),
            (1, 1) => shared.bank_registers[7] as usize,
            (1, 2) => shared.bank_registers[6] as usize,
            (1, 3) => self.num_prg_banks.saturating_sub(1),
            _ => 0,
        };

        // Wrap bank index within available banks
        bank_index % self.num_prg_banks
    }

    /// Handle write to MMC3 registers ($8000-$FFFF)
    fn write_register(&mut self, addr: u16, value: u8) {
        let mut shared = self.shared.borrow_mut();

        match addr & 0xE001 {
            // Bank select ($8000, even)
            0x8000 => {
                shared.bank_register_index = (value & 0x07) as usize;
                shared.prg_mode = (value >> 6) & 0x01;
                shared.chr_mode = (value >> 7) & 0x01;
                trace!("MMC3: Bank select - index={}, prg_mode={}, chr_mode={}",
                       shared.bank_register_index, shared.prg_mode, shared.chr_mode);
            }
            // Bank data ($8001, odd)
            0x8001 => {
                let idx = shared.bank_register_index;
                shared.bank_registers[idx] = value;
                trace!("MMC3: Bank R{} = {}", idx, value);
            }
            // Mirroring ($A000, even)
            0xA000 => {
                drop(shared); // Release borrow before borrowing mirroring
                if !self.mirroring_fixed {
                    let new_mirroring = if value & 0x01 == 0 {
                        PpuNameTableMirroring::Vertical
                    } else {
                        PpuNameTableMirroring::Horizontal
                    };
                    *self.mirroring.borrow_mut() = new_mirroring;
                    trace!("MMC3: Mirroring = {:?}", new_mirroring);
                }
            }
            // PRG-RAM protect ($A001, odd)
            0xA001 => {
                drop(shared); // Release borrow
                self.prg_ram_enabled = (value & 0x80) != 0;
                self.prg_ram_write_protect = (value & 0x40) != 0;
                trace!("MMC3: PRG-RAM enabled={}, write_protect={}",
                       self.prg_ram_enabled, self.prg_ram_write_protect);
            }
            // IRQ latch ($C000, even)
            0xC000 => {
                shared.irq_latch = value;
                trace!("MMC3: IRQ latch = {}", value);
            }
            // IRQ reload ($C001, odd)
            0xC001 => {
                shared.irq_reload = true;
                trace!("MMC3: IRQ reload triggered");
            }
            // IRQ disable ($E000, even)
            0xE000 => {
                shared.irq_enabled = false;
                shared.irq_pending.set(false);
                trace!("MMC3: IRQ disabled and cleared");
            }
            // IRQ enable ($E001, odd)
            0xE001 => {
                shared.irq_enabled = true;
                trace!("MMC3: IRQ enabled");
            }
            _ => {}
        }
    }

    /// Check if IRQ is pending
    pub fn poll_irq(&self) -> bool {
        self.shared.borrow().irq_pending.get()
    }

    /// Clear the IRQ pending flag (call after acknowledging IRQ)
    pub fn clear_irq(&self) {
        self.shared.borrow().irq_pending.set(false);
    }
}

impl FromINes for Mmc3Cartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<Mmc3Cartridge, LoaderError>
    where
        Self: Sized,
    {
        let has_prg_ram = header.prg_ram_size > 0 || header.battery;
        let mirroring_fixed = header.alternative_nametables;

        let cartridge = Mmc3Cartridge::build(
            file,
            header.prg_offset(),
            header.prg_rom_size,
            header.chr_rom_size,
            header.chr_ram_size,
            has_prg_ram,
            header.nametables_layout,
            mirroring_fixed,
            header.region,
        )?;

        debug!("MMC3: Loaded cartridge - {} PRG banks, {} CHR banks, region={:?}",
               cartridge.num_prg_banks, cartridge.shared.borrow().num_chr_banks, header.region);

        Ok(cartridge)
    }
}

impl Memory for Mmc3Cartridge {
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        match addr {
            // PRG-RAM ($6000-$7FFF)
            0x6000..=0x7FFF => {
                if let Some(ref prg_ram) = self.prg_ram {
                    if self.prg_ram_enabled {
                        prg_ram.borrow().read_byte(addr - 0x6000)
                    } else {
                        // PRG-RAM disabled - return open bus
                        Ok(0xFF)
                    }
                } else {
                    // No PRG-RAM - return open bus
                    Ok(0xFF)
                }
            }
            // PRG-ROM ($8000-$FFFF)
            0x8000..=0xFFFF => {
                let bank_index = self.get_prg_bank_for_address(addr);
                let offset = (addr - 0x8000) % MMC3_PRG_BANK_SIZE as u16;
                self.prg_rom_banks[bank_index].read_byte(offset)
            }
            _ => Err(MemoryError::OutOfRange(addr)),
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        match addr {
            // PRG-RAM ($6000-$7FFF)
            0x6000..=0x7FFF => {
                if let Some(ref prg_ram) = self.prg_ram {
                    if self.prg_ram_enabled && !self.prg_ram_write_protect {
                        prg_ram.borrow_mut().write_byte(addr - 0x6000, value)?;
                    }
                    // If disabled or write-protected, silently ignore
                }
                Ok(())
            }
            // MMC3 registers ($8000-$FFFF)
            0x8000..=0xFFFF => {
                self.write_register(addr, value);
                Ok(())
            }
            _ => Err(MemoryError::OutOfRange(addr)),
        }
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.read_byte(addr)? as u16;
        let hi = self.read_byte(addr.wrapping_add(1))? as u16;
        Ok((hi << 8) | lo)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.write_byte(addr, (value & 0xFF) as u8)?;
        self.write_byte(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8)
    }

    fn size(&self) -> usize {
        self.num_prg_banks * MMC3_PRG_BANK_SIZE
    }
}

impl BusDevice for Mmc3Cartridge {
    fn get_name(&self) -> String {
        format!("{} Cartridge", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        (0x6000, 0xFFFF)
    }
}

/// CHR memory wrapper for MMC3 that handles bank switching and A12 detection
#[derive(Debug)]
pub struct Mmc3ChrMemory {
    /// Reference to shared state for bank switching and IRQ
    shared: Rc<RefCell<Mmc3SharedState>>,
}

impl Mmc3ChrMemory {
    pub fn new(shared: Rc<RefCell<Mmc3SharedState>>) -> Self {
        Self { shared }
    }
}

impl Memory for Mmc3ChrMemory {
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let mut shared = self.shared.borrow_mut();

        // Notify A12 for IRQ counter
        shared.notify_ppu_address(addr);

        let bank_index = shared.get_chr_bank_for_address(addr);
        let offset = addr % MMC3_CHR_BANK_SIZE as u16;
        shared.chr_banks[bank_index].read_byte(offset)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let mut shared = self.shared.borrow_mut();

        // Notify A12 for IRQ counter
        shared.notify_ppu_address(addr);

        // Only write if CHR-RAM
        if !shared.chr_is_rom {
            let bank_index = shared.get_chr_bank_for_address(addr);
            let offset = addr % MMC3_CHR_BANK_SIZE as u16;
            shared.chr_banks[bank_index].write_byte(offset, value)?;
        }
        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.read_byte(addr)? as u16;
        let hi = self.read_byte(addr.wrapping_add(1))? as u16;
        Ok((hi << 8) | lo)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.write_byte(addr, (value & 0xFF) as u8)?;
        self.write_byte(addr.wrapping_add(1), ((value >> 8) & 0xFF) as u8)
    }

    fn size(&self) -> usize {
        self.shared.borrow().num_chr_banks * MMC3_CHR_BANK_SIZE
    }
}

impl BusDevice for Mmc3ChrMemory {
    fn get_name(&self) -> String {
        format!("{} CHR Memory", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::CARTRIDGE(CartridgeType::MMC3)
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_ADDRESS_SPACE
    }
}

impl Cartridge for Mmc3Cartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_memory.clone()
    }

    fn get_prg_ram(&self) -> Option<Rc<RefCell<dyn BusDevice>>> {
        self.prg_ram.as_ref().map(|ram| ram.clone() as Rc<RefCell<dyn BusDevice>>)
    }

    fn get_mirroring(&self) -> Rc<RefCell<PpuNameTableMirroring>> {
        self.mirroring.clone()
    }

    fn get_region(&self) -> Region {
        self.region
    }

    fn poll_irq(&self) -> bool {
        self.shared.borrow().irq_pending.get()
    }

    fn clear_irq(&self) {
        self.shared.borrow().irq_pending.set(false);
    }

    fn notify_ppu_address(&self, addr: u16) {
        self.shared.borrow_mut().notify_ppu_address(addr);
    }
}

#[cfg(test)]
impl Mmc3Cartridge {
    /// Test helper: get current PRG mode
    pub fn get_prg_mode(&self) -> u8 {
        self.shared.borrow().prg_mode
    }

    /// Test helper: get current CHR mode
    pub fn get_chr_mode(&self) -> u8 {
        self.shared.borrow().chr_mode
    }

    /// Test helper: get bank register value
    pub fn get_bank_register(&self, index: usize) -> u8 {
        self.shared.borrow().bank_registers[index]
    }

    /// Test helper: get IRQ counter
    pub fn get_irq_counter(&self) -> u8 {
        self.shared.borrow().irq_counter
    }

    /// Test helper: get IRQ latch
    pub fn get_irq_latch(&self) -> u8 {
        self.shared.borrow().irq_latch
    }

    /// Test helper: check if IRQ enabled
    pub fn is_irq_enabled(&self) -> bool {
        self.shared.borrow().irq_enabled
    }

    /// Test helper: check if PRG-RAM enabled
    pub fn is_prg_ram_enabled(&self) -> bool {
        self.prg_ram_enabled
    }

    /// Test helper: check if PRG-RAM write protected
    pub fn is_prg_ram_write_protected(&self) -> bool {
        self.prg_ram_write_protect
    }

    /// Test helper: notify PPU address for A12 (for IRQ tests)
    pub fn notify_ppu_address(&mut self, addr: u16) {
        self.shared.borrow_mut().notify_ppu_address(addr);
    }
}
