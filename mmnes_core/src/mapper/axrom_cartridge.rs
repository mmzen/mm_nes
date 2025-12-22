// Authorship: Human 0% | Claude 100%
//! AxROM (Mapper 7) cartridge implementation.
//!
//! AxROM is used by games like Battletoads, Marble Madness, and RC Pro-Am.
//!
//! ## PRG-ROM Banking
//! - Single 32KB window at $8000-$FFFF (fully switchable)
//! - Bank select via bits 0-3 of any write to $8000-$FFFF
//!
//! ## Mirroring
//! - One-screen mirroring only (no horizontal/vertical)
//! - Bit 4 of bank register: 0 = SingleScreenLower, 1 = SingleScreenUpper
//! - Mirroring can change at runtime on every write
//!
//! ## CHR Memory
//! - CHR-RAM only (8KB, no banking)
//! - CHR-ROM is not supported; ROMs with CHR-ROM are rejected
//!
//! ## PRG-RAM
//! - Optional 8KB at $6000-$7FFF (if header specifies)

use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, CartridgeType, CPU_ADDRESS_SPACE, PPU_ADDRESS_SPACE};
use crate::ines_loader::{FromINes, INesRomHeader, Region};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::memory_ciram::PpuNameTableMirroring;

/// AxROM uses 32KB PRG-ROM banks
const AXROM_PRG_MEMORY_BANK_SIZE: usize = 32 * 1024;

/// AxROM uses 8KB CHR-RAM (no banking)
const AXROM_CHR_MEMORY_BANK_SIZE: usize = 8 * 1024;

/// PRG-RAM address space ($6000-$7FFF)
const PRG_RAM_ADDRESS_SPACE: (u16, u16) = (0x6000, 0x7FFF);

/// PRG-RAM bank size (8KB)
const PRG_RAM_BANK_SIZE: usize = 8 * 1024;

const MAPPER_NAME: &str = "AXROM";

#[derive(Debug)]
pub struct AxromCartridge {
    /// PRG-ROM banks (32KB each)
    memory_banks: Vec<MemoryBank>,
    /// Currently selected PRG-ROM bank
    current_bank: usize,
    /// Total number of PRG-ROM banks
    num_memory_banks: usize,
    /// PRG-ROM window size (always 32KB for bus masking)
    prg_rom_size: usize,
    /// 8KB CHR-RAM
    chr_ram: Rc<RefCell<MemoryBank>>,
    /// Optional 8KB PRG-RAM at $6000-$7FFF
    prg_ram: Option<Rc<RefCell<MemoryBank>>>,
    /// Nametable mirroring (one-screen, controllable at runtime)
    mirroring: Rc<RefCell<PpuNameTableMirroring>>,
    /// Bus device type identifier
    device_type: BusDeviceType,
    /// Console region (NTSC/PAL)
    region: Region,
}

impl AxromCartridge {
    /// Create a new AxROM cartridge.
    ///
    /// # Arguments
    /// * `data` - File reader positioned at start of ROM data
    /// * `prg_rom_offset` - Offset to PRG-ROM in file
    /// * `prg_rom_size` - Total PRG-ROM size in bytes
    /// * `chr_rom_size` - CHR-ROM size (must be 0 for AxROM)
    /// * `chr_ram_size` - CHR-RAM size (typically 8KB)
    /// * `prg_ram_size` - PRG-RAM size (0 or 8KB)
    /// * `mirroring` - Initial mirroring from header (will be overridden by mapper writes)
    /// * `region` - Console region
    ///
    /// # Errors
    /// Returns `CartridgeError::Unsupported` if CHR-ROM is present (AxROM uses CHR-RAM only)
    pub fn new(
        mut data: BufReader<File>,
        prg_rom_offset: u64,
        prg_rom_size: usize,
        chr_rom_size: usize,
        chr_ram_size: usize,
        prg_ram_size: usize,
        mirroring: PpuNameTableMirroring,
        region: Region,
    ) -> Result<AxromCartridge, CartridgeError> {
        // AxROM does not support CHR-ROM
        if chr_rom_size > 0 {
            return Err(CartridgeError::Unsupported(
                "AxROM does not support CHR-ROM; only CHR-RAM is allowed".to_string()
            ));
        }

        // Load PRG-ROM banks (32KB each)
        let prg_memory_banks = cartridge::create_prg_rom_memory(
            &mut data,
            prg_rom_offset,
            prg_rom_size,
            AXROM_PRG_MEMORY_BANK_SIZE,
            CPU_ADDRESS_SPACE,
        )?;
        let num_memory_banks = prg_memory_banks.len();
        debug!(
            "AXROM: prg rom size: {}, number of banks: {}",
            prg_rom_size, num_memory_banks
        );

        // Create CHR-RAM (always 8KB for AxROM)
        let chr_ram_actual_size = if chr_ram_size > 0 { chr_ram_size } else { AXROM_CHR_MEMORY_BANK_SIZE };
        let chr_ram_banks = cartridge::create_chr_ram_memory(
            chr_ram_actual_size,
            AXROM_CHR_MEMORY_BANK_SIZE,
            PPU_ADDRESS_SPACE,
        )?;
        let chr_ram = cartridge::get_first_bank_or_fail(
            chr_ram_banks,
            chr_ram_actual_size,
            AXROM_CHR_MEMORY_BANK_SIZE,
            false, // is_rom = false (it's RAM)
        )?;
        debug!("AXROM: chr ram size: {}", chr_ram_actual_size);

        // Create PRG-RAM if specified in header
        let prg_ram = if prg_ram_size > 0 {
            let prg_ram_banks = cartridge::create_prg_ram_memory(
                prg_ram_size,
                PRG_RAM_BANK_SIZE,
                PRG_RAM_ADDRESS_SPACE,
            )?;
            let prg_ram_bank = cartridge::get_first_bank_or_fail(
                prg_ram_banks,
                prg_ram_size,
                PRG_RAM_BANK_SIZE,
                false,
            )?;
            debug!("AXROM: prg ram size: {}", prg_ram_size);
            Some(Rc::new(RefCell::new(prg_ram_bank)))
        } else {
            debug!("AXROM: no prg ram");
            None
        };

        // AxROM uses one-screen mirroring; convert header mirroring to one-screen
        // Initial state: SingleScreenLower (bit 4 = 0)
        let initial_mirroring = match mirroring {
            PpuNameTableMirroring::SingleScreenLower | PpuNameTableMirroring::SingleScreenUpper => mirroring,
            _ => PpuNameTableMirroring::SingleScreenLower,
        };

        let cartridge = AxromCartridge {
            memory_banks: prg_memory_banks,
            current_bank: 0,
            num_memory_banks,
            prg_rom_size: (CPU_ADDRESS_SPACE.1 - CPU_ADDRESS_SPACE.0 + 1) as usize,
            chr_ram: Rc::new(RefCell::new(chr_ram)),
            prg_ram,
            mirroring: Rc::new(RefCell::new(initial_mirroring)),
            device_type: BusDeviceType::CARTRIDGE(CartridgeType::AXROM),
            region,
        };

        Ok(cartridge)
    }

    fn build(
        file: File,
        prg_rom_offset: u64,
        prg_rom_size: usize,
        chr_rom_size: usize,
        chr_ram_size: usize,
        prg_ram_size: usize,
        mirroring: PpuNameTableMirroring,
        region: Region,
    ) -> Result<AxromCartridge, LoaderError> {
        debug!("creating AXROM cartridge");

        let reader = BufReader::new(file);
        let cartridge = AxromCartridge::new(
            reader,
            prg_rom_offset,
            prg_rom_size,
            chr_rom_size,
            chr_ram_size,
            prg_ram_size,
            mirroring,
            region,
        )?;

        Ok(cartridge)
    }
}

impl FromINes for AxromCartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<AxromCartridge, LoaderError>
    where
        Self: Sized,
    {
        let cartridge = AxromCartridge::build(
            file,
            header.prg_offset(),
            header.prg_rom_size,
            header.chr_rom_size,
            header.chr_ram_size,
            header.prg_ram_size,
            header.nametables_layout,
            header.region,
        )?;

        Ok(cartridge)
    }
}

impl Memory for AxromCartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        // AxROM: entire $8000-$FFFF is a single 32KB switchable bank
        // Mask address to 15 bits (32KB)
        let masked_addr = addr & 0x7FFF;
        debug!(
            "AXROM: reading byte from bank {} at 0x{:04X} (original: 0x{:04X})",
            self.current_bank, masked_addr, addr
        );
        self.memory_banks[self.current_bank].read_byte(masked_addr)
    }

    fn peek_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, _addr: u16, value: u8) -> Result<(), MemoryError> {
        // AxROM bank register format:
        // Bits 0-3: PRG-ROM bank select
        // Bit 4: Mirroring (0 = SingleScreenLower, 1 = SingleScreenUpper)
        // Bits 5-7: Unused

        let previous_bank = self.current_bank;
        let previous_mirroring = *self.mirroring.borrow();

        // Bank select: bits 0-3, modulo number of banks
        self.current_bank = (value & 0x0F) as usize % self.num_memory_banks;

        // Mirroring select: bit 4
        let new_mirroring = if value & 0x10 != 0 {
            PpuNameTableMirroring::SingleScreenUpper
        } else {
            PpuNameTableMirroring::SingleScreenLower
        };
        *self.mirroring.borrow_mut() = new_mirroring;

        debug!(
            "AXROM: write 0x{:02X} - bank: {} -> {}, mirroring: {:?} -> {:?}",
            value, previous_bank, self.current_bank, previous_mirroring, new_mirroring
        );

        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let masked_addr = addr & 0x7FFF;
        debug!(
            "AXROM: reading word from bank {} at 0x{:04X} (original: 0x{:04X})",
            self.current_bank, masked_addr, addr
        );
        self.memory_banks[self.current_bank].read_word(masked_addr)
    }

    fn write_word(&mut self, _addr: u16, _value: u16) -> Result<(), MemoryError> {
        // PRG-ROM is not writable; writes update mapper state via write_byte
        unreachable!()
    }

    fn dump(&self) {
        unimplemented!()
    }

    fn size(&self) -> usize {
        self.prg_rom_size
    }
}

impl BusDevice for AxromCartridge {
    fn get_name(&self) -> String {
        format!("{}", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        CPU_ADDRESS_SPACE
    }
}

impl Cartridge for AxromCartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_ram.clone()
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
}

#[cfg(test)]
impl AxromCartridge {
    /// Test helper: get current bank
    pub fn get_current_bank(&self) -> usize {
        self.current_bank
    }

    /// Test helper: get number of banks
    pub fn get_num_banks(&self) -> usize {
        self.num_memory_banks
    }

    /// Test helper: get current mirroring
    pub fn get_current_mirroring(&self) -> PpuNameTableMirroring {
        *self.mirroring.borrow()
    }
}
