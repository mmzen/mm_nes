use std::cell::RefCell;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::rc::Rc;
use log::debug;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, PPU_ADDRESS_SPACE};
use crate::cartridge::CartridgeType::MMC1;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const MMC1_PRG_MEMORY_BANK_SIZE: usize = 16 * 1024;
const MMC1_CHR_MEMORY_BANK_SIZE: usize = 4 * 1024;
const MAPPER_NAME: &str = "MMC1";


/***
 * a changer probablement: ne plus transmettre chr_rom au bus, mais
 * faire en sorte que cartridge soit directement connecté au bus pour à la fois
 * la mémoire chr mais aussi prg
 *
 ***/

/***
 * https://www.nesdev.org/wiki/MMC1
 * SxROM board types
 */

#[derive(Debug)]
struct SwitchableMemory {
    size: usize,
    memory_banks: Vec<MemoryBank>,
    num_memory_banks: usize,
    current_bank: usize,
    fixed_bank: usize,
}

#[derive(Debug)]
pub struct Mmc1Cartridge {
    shift_register: u8,
    prg_rom: SwitchableMemory,
    prg_ram: SwitchableMemory,
    chr_rom: SwitchableMemory,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring
}

impl Mmc1Cartridge {

    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize, prg_ram_size: usize,
               chr_rom_offset: u64, chr_rom_size: usize, chr_ram_size: usize,
               mirroring: PpuNameTableMirroring) -> Result<Mmc1Cartridge, CartridgeError> {

        let (prg_rom_memory_banks, prg_rom_num_memory_banks) = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, MMC1_PRG_MEMORY_BANK_SIZE, CPU_ADDRESS_SPACE)?;
        let prog_rom_fixed_bank = prg_rom_num_memory_banks - 1;

        let (chr_memory_size, is_chr_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);
        let rom_data = if is_chr_rom { Some(&mut data) } else { None };

        let (chr_memory_banks, num_chr_banks) = cartridge::create_chr_memory(rom_data, chr_rom_offset, chr_memory_size, MMC1_CHR_MEMORY_BANK_SIZE, is_chr_rom, PPU_ADDRESS_SPACE)?;

        let (prg_ram_memory_banks, prg_ram_num_memory_banks) = if prg_ram_size > 0 {
            cartridge::create_prg_ram_memory(prg_ram_size, MMC1_PRG_MEMORY_BANK_SIZE, CPU_ADDRESS_SPACE)?
        } else {
            (Vec::new(), 0)
        };
        let prog_ram_fixed_bank = if prg_ram_num_memory_banks > 0 { prg_ram_num_memory_banks - 1 } else { 0 };

        let cartridge = Mmc1Cartridge {
            shift_register: 0,
            prg_rom: SwitchableMemory {
                size: MMC1_PRG_MEMORY_BANK_SIZE,
                memory_banks: prg_rom_memory_banks,
                num_memory_banks: prg_rom_num_memory_banks,
                current_bank: 0,
                fixed_bank: prog_rom_fixed_bank,
            },
            prg_ram: SwitchableMemory {
                size: MMC1_PRG_MEMORY_BANK_SIZE,
                memory_banks: prg_ram_memory_banks,
                num_memory_banks: prg_ram_num_memory_banks,
                current_bank: 0,
                fixed_bank: prog_ram_fixed_bank,
            },
            chr_rom: SwitchableMemory {
                size: chr_rom_size,
                memory_banks: chr_memory_banks,
                num_memory_banks: num_chr_banks,
                current_bank: 0,
                fixed_bank: 0,
            },
            device_type: BusDeviceType::CARTRIDGE(MMC1),
            mirroring,
        };

        Ok(cartridge)
    }


    fn build(file: File,
             prg_rom_offset: u64, prg_rom_size: usize, prg_ram_size: usize,
             chr_rom_offset: Option<u64>, chr_rom_size: usize, _chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<Mmc1Cartridge, LoaderError> {
        debug!("creating MMC1 cartridge");

        let reader = BufReader::new(file);
        let chr_rom_offset = if let Some(chr_rom_offset_unwrapped) = chr_rom_offset { chr_rom_offset_unwrapped } else { 0 };

        let cartridge = Mmc1Cartridge::new(reader, prg_rom_offset, prg_rom_size, prg_ram_size, chr_rom_offset, chr_rom_size, chr_rom_size, mirroring)?;
        Ok(cartridge)
    }
}

impl FromINes for Mmc1Cartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<Mmc1Cartridge, LoaderError>
    where
        Self: Sized
    {
        let cartridge = Mmc1Cartridge::build(file,
                                              header.prg_offset(), header.prg_rom_size, header.prg_ram_size,
                                              header.chr_offset(), header.chr_rom_size, header.chr_ram_size,
                                              header.nametables_layout)?;

        Ok(cartridge)
    }
}

impl BusDevice for Mmc1Cartridge {
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

impl Memory for Mmc1Cartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        todo!()
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        todo!()
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        todo!()
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        unreachable!()
    }

    fn dump(&self) {
        unimplemented!()
    }

    fn size(&self) -> usize {
        self.prg_rom.size
    }
}

impl Cartridge for Mmc1Cartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        let p = MemoryBank::new(0, (0, 0));
        Rc::new(RefCell::new(p))
    }

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring.clone()
    }
}