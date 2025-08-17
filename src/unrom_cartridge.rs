use std::cell::RefCell;
use std::fs::File;
use std::io::{BufReader, Seek, SeekFrom};
use std::rc::Rc;
use log::{debug, info};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError};
use crate::cartridge::CartridgeType::UNROM;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::ppu::PpuNameTableMirroring;

const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const PPU_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x1FFF);
const MEMORY_BANK_SIZE: usize = 16 * 1024;
const MEMORY_FIXED_BANK_PHYS_ADDR: u16 = 0xC000 - CPU_ADDRESS_SPACE.0;

const MAPPER_NAME: &str = "UNROM";

#[derive(Debug)]
pub struct UnromCartridge {
    memory_banks: Vec<MemoryBank>,
    current_bank: usize,
    fixed_bank: usize,
    num_memory_banks: usize,
    chr_rom: Rc<RefCell<MemoryBank>>,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring
}

impl UnromCartridge {
    
    fn create_chr_memory(data: &mut BufReader<File>, chr_rom_addr: u64, chr_rom_size: usize) -> Result<MemoryBank, CartridgeError> {
        let mut chr_rom = MemoryBank::new(chr_rom_size, PPU_ADDRESS_SPACE);

        debug!("UNROM: loading chr_rom data ({} KB)...", chr_rom_size / 1024);
        data.seek(SeekFrom::Start(chr_rom_addr))?;
        cartridge::write_rom_data(&mut chr_rom, chr_rom_size, data)?;
        
        Ok(chr_rom)
    }
    
    fn create_prg_memory(data: &mut BufReader<File>, prg_rom_addr: u64, prg_rom_size: usize) -> Result<(Vec<MemoryBank>, usize), CartridgeError> {
        let num_memory_banks = prg_rom_size / MEMORY_BANK_SIZE;
        let mut memory_banks = Vec::with_capacity(num_memory_banks);

        for bank in 0..num_memory_banks {
            debug!("UNROM: loading prg_rom data ({} / {} KB ) in memory bank {} / {}...",
                MEMORY_BANK_SIZE, prg_rom_size, bank + 1, num_memory_banks);

            let mut prg_rom = MemoryBank::new(prg_rom_size, CPU_ADDRESS_SPACE);

            data.seek(SeekFrom::Start(prg_rom_addr + ((bank * MEMORY_BANK_SIZE) as u64)))?;
            cartridge::write_rom_data(&mut prg_rom, prg_rom_size, data)?;

            memory_banks.push(prg_rom);
        }

        Ok((memory_banks, num_memory_banks))
    }

    pub fn new(mut data: BufReader<File>,
               prg_rom_addr: u64, prg_rom_size: usize,
               chr_rom_addr: u64, chr_rom_size: usize, mirroring: PpuNameTableMirroring) -> Result<UnromCartridge, CartridgeError> {

        
        let (memory_banks, num_memory_banks) = UnromCartridge::create_prg_memory(&mut data, prg_rom_addr, prg_rom_size)?;
        let fixed_bank = num_memory_banks - 1;
        
        let chr_rom = UnromCartridge::create_chr_memory(&mut data, chr_rom_addr, chr_rom_size)?;
        
        let cartridge = UnromCartridge {
            memory_banks,
            current_bank: 0,
            fixed_bank,
            num_memory_banks,
            device_type: BusDeviceType::CARTRIDGE(UNROM),
            mirroring,
            chr_rom: Rc::new(RefCell::new(chr_rom)),
        };

        Ok(cartridge)
    }

    fn build(file: File,
             prg_rom_addr: u64, prg_rom_size: usize,
             chr_rom_addr: u64, chr_rom_size: usize, mirroring: PpuNameTableMirroring) -> Result<UnromCartridge, LoaderError> {
        info!("creating UNROM cartridge");

        let reader = BufReader::new(file);
        let cartridge = UnromCartridge::new(reader, prg_rom_addr, prg_rom_size, chr_rom_addr, chr_rom_size, mirroring)?;
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
                                             header.prg_addr(), header.prg_rom_size,
                                             header.chr_addr(), header.chr_rom_size,
                                             header.nametables_layout)?;

        Ok(cartridge)
    }
}

impl Memory for UnromCartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        if addr >= MEMORY_FIXED_BANK_PHYS_ADDR {
            self.memory_banks[self.fixed_bank].read_byte(addr)
        } else {
            self.memory_banks[self.current_bank].read_byte(addr)
        }
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, _: u16, value: u8) -> Result<(), MemoryError> {
        self.current_bank = (value & 0x03) as usize % self.num_memory_banks ;
        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        unimplemented!()
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        unimplemented!()
    }

    fn dump(&self) {
        unimplemented!()
    }

    fn size(&self) -> usize {
        unimplemented!()
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