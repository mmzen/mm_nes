use std::cell::RefCell;
use std::fs::File;
use std::io::{BufReader};
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, CPU_ADDRESS_SPACE, PPU_ADDRESS_SPACE, PRG_MEMORY_BANK_SIZE};
use crate::cartridge::CartridgeType::UNROM;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

pub const CHR_MEMORY_BANK_SIZE: usize = 8 * 1024;
const MEMORY_FIXED_BANK_PHYS_ADDR: u16 = 0x3FFF; // 0xFFFF - 0x4000 (16 KB);
const MAPPER_NAME: &str = "UNROM";

#[derive(Debug)]
pub struct UnromCartridge {
    memory_banks: Vec<MemoryBank>,
    current_bank: usize,
    fixed_bank: usize,
    num_memory_banks: usize,
    prg_rom_size: usize,
    chr_rom: Rc<RefCell<MemoryBank>>,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring
}

impl UnromCartridge {

    /***
     * the cartridge announce a 32 Kb memory, to be sure to catch the high memory reads, served by a fixed bank.
     * the underlying memory mapping is made by multiple 16 KB memory banks, switched by writes.
     * https://www.nesdev.org/wiki/UxROM
     ***/
    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize,
               chr_rom_offset: u64, chr_rom_size: usize,
               chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<UnromCartridge, CartridgeError> {


        let (prg_memory_banks, prg_num_memory_banks) = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, PRG_MEMORY_BANK_SIZE, CPU_ADDRESS_SPACE)?;
        let prg_fixed_bank = prg_num_memory_banks - 1;
        debug!("UNROM: prg rom size: {}, number of bank: {}, fixed bank {}", prg_rom_size, prg_num_memory_banks, prg_fixed_bank);

        let (chr_memory_size, is_chr_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);
        let rom_data = if is_chr_rom { Some(&mut data) } else { None };
        let (mut chr_memory_banks, num_chr_banks) = cartridge::create_chr_memory(rom_data, chr_rom_offset, chr_memory_size, CHR_MEMORY_BANK_SIZE, is_chr_rom, PPU_ADDRESS_SPACE)?;
        debug!("UNROM: chr memory size: {}, number of bank: {}, ram: {}", chr_memory_size, num_chr_banks, !is_chr_rom);

        let chr_memory_bank = if let Some(bank) = chr_memory_banks.pop() && num_chr_banks == 1 {
            bank
        } else {
            Err(CartridgeError::LoadingError(
                format!("error while creating chr memory bank, total size: {}, bank size: {}, number of banks: {}, is ram: {}, banks in array: {}",
                        chr_ram_size, CHR_MEMORY_BANK_SIZE, num_chr_banks, !is_chr_rom, chr_memory_banks.len())
            ))?
        };

        let cartridge = UnromCartridge {
            memory_banks: prg_memory_banks,
            current_bank: 0,
            fixed_bank: prg_fixed_bank,
            num_memory_banks: prg_num_memory_banks,
            prg_rom_size: (CPU_ADDRESS_SPACE.1 - CPU_ADDRESS_SPACE.0 + 1) as usize,
            device_type: BusDeviceType::CARTRIDGE(UNROM),
            mirroring,
            chr_rom: Rc::new(RefCell::new(chr_memory_bank)),
        };

        Ok(cartridge)
    }

    fn build(file: File,
             prg_rom_offset: u64, prg_rom_size: usize,
             chr_rom_offset: Option<u64>, chr_rom_size: usize, chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<UnromCartridge, LoaderError> {
        debug!("creating UNROM cartridge");

        let reader = BufReader::new(file);
        let chr_rom_offset = if let Some(chr_rom_offset_unwrapped) = chr_rom_offset { chr_rom_offset_unwrapped } else { 0 };

        let cartridge = UnromCartridge::new(reader, prg_rom_offset, prg_rom_size, chr_rom_offset, chr_rom_size, chr_ram_size, mirroring)?;
        Ok(cartridge)
    }
}

impl FromINes for UnromCartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<UnromCartridge, LoaderError>
    where
        Self: Sized
    {
        let cartridge = UnromCartridge::build(file,
                                              header.prg_offset(), header.prg_rom_size,
                                              header.chr_offset(), header.chr_rom_size,
                                              header.chr_ram_size, header.nametables_layout)?;

        Ok(cartridge)
    }
}

impl Memory for UnromCartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        if addr > MEMORY_FIXED_BANK_PHYS_ADDR {
            let remapped_addr = addr & 0x3FFF;
            debug!("UNROM: reading byte from fixed bank at 0x{:04X} (initial addr: 0x{:04X}), bank: {}, ", remapped_addr, addr, self.fixed_bank);
            self.memory_banks[self.fixed_bank].read_byte(remapped_addr)
        } else {
            debug!("UNROM: reading byte from switchable bank at 0x{:04X}, bank: {}", addr, self.current_bank);
            self.memory_banks[self.current_bank].read_byte(addr)
        }
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        let previous_bank = self.current_bank;
        self.current_bank = (value & 0x0F) as usize % self.num_memory_banks;
        debug!("UNROM: switching to bank: was: {}, now: {} (raw write: 0x{:04X})", previous_bank, self.current_bank, value);
        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        if addr > MEMORY_FIXED_BANK_PHYS_ADDR {
            let remapped_addr = addr & 0x3FFF;
            debug!("UNROM: reading word from fixed bank at 0x{:04X} (initial addr: 0x{:04X}), bank: {}, ", remapped_addr, addr, self.fixed_bank);
            self.memory_banks[self.fixed_bank].read_word(remapped_addr)
        } else {
            debug!("UNROM: reading word from switchable bank at 0x{:04X}, bank: {}", addr, self.current_bank);
            self.memory_banks[self.current_bank].read_word(addr)
        }
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        unreachable!()
    }

    fn dump(&self) {
        unimplemented!()
    }

    fn size(&self) -> usize {
        self.prg_rom_size
    }
}

impl BusDevice for UnromCartridge {
    fn get_name(&self) -> String {
        format!("{}", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_address_range(&self) -> (u16, u16) {
        CPU_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        CPU_ADDRESS_SPACE.0 <= addr && addr <= CPU_ADDRESS_SPACE.1
    }
}

impl Cartridge for UnromCartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_rom.clone()
    }

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring.clone()
    }
}