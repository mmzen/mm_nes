use std::cell::{Cell, RefCell};
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use log::info;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, PPU_ADDRESS_SPACE};
use crate::cartridge::CartridgeType::MMC2;
use crate::ines_loader::{FromINes, INesRomHeader, Region};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_bank::MemoryBank;
use crate::memory_ciram::PpuNameTableMirroring;

const PRG_ROM_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const PRG_RAM_ADDRESS_SPACE: (u16, u16) = (0x6000, 0x7FFF);
const MMC2_PRG_ROM_BANK_SIZE: usize = 8 * 1024;
const MMC2_PRG_RAM_BANK_SIZE: usize = 8 * 1024;
const MMC2_CHR_MEMORY_BANK_SIZE: usize = 4 * 1024;
const MAPPER_NAME: &str = "MMC2";


#[derive(Debug, Default, PartialEq, Copy, Clone)]
enum Latch {
    FD,
    #[default]
    FE
}

impl Latch {

    fn index(&self) -> usize {
        match self {
            Latch::FD => 0,
            Latch::FE => 1,
        }
    }
}

#[derive(Debug)]
struct Mmc2PrgRom {
    size: usize,
    memory_banks: Vec<MemoryBank>,
    memory_bank_map: [usize; 4],
}

impl Mmc2PrgRom {
    fn new(memory_banks: Vec<MemoryBank>) -> Result<Mmc2PrgRom, CartridgeError> {
        let num_memory_banks = memory_banks.len();

        if num_memory_banks < 4 {
            Err(CartridgeError::IllegalState(format!("number of memory banks must be at least 4: {}", num_memory_banks)))
        } else {
            let memory_bank_map = [0, memory_banks.len() - 3, memory_banks.len() - 2, memory_banks.len() - 1];

            let prg_rom = Mmc2PrgRom {
                size: (PRG_ROM_ADDRESS_SPACE.1 - PRG_ROM_ADDRESS_SPACE.0 + 1) as usize,
                memory_banks,
                memory_bank_map
            };

            Ok(prg_rom)
        }
    }
}

impl Memory for Mmc2PrgRom {
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {

        let index = match addr {
            0x0000..=0x1FFF => self.memory_bank_map[0],
            0x2000..=0x3FFF => self.memory_bank_map[1],
            0x4000..=0x5FFF => self.memory_bank_map[2],
            0x6000..=0x7FFF => self.memory_bank_map[3],
            _ => panic!("invalid address: 0x{:04X}", addr),
        };

        let remapped_addr = addr & 0x1FFF;
        self.memory_banks[index].read_byte(remapped_addr)
    }

    fn size(&self) -> usize {
        self.size
    }
}

#[derive(Debug)]
struct Mmc2ChrRom {
    size: usize,
    memory_banks: Vec<MemoryBank>,
    memory_bank_map_0x0000: [usize; 2],
    memory_bank_map_0x1000: [usize; 2],
    latch0: Cell<Latch>,
    latch1: Cell<Latch>,
}

impl Mmc2ChrRom {

    fn new(memory_banks: Vec<MemoryBank>) -> Mmc2ChrRom {

        Mmc2ChrRom {
            size: (PPU_ADDRESS_SPACE.1 - PPU_ADDRESS_SPACE.0 + 1) as usize,
            memory_banks,
            memory_bank_map_0x0000: [0; 2],
            memory_bank_map_0x1000: [0; 2],
            latch0: Cell::new(Latch::default()),
            latch1: Cell::new(Latch::default()),
        }
    }

    #[inline]
    fn update_latch_on_read(&self, addr: u16) {
        match addr {
            0x0FD8 => if self.latch0.get() != Latch::FD { self.latch0.set(Latch::FD) },
            0x0FE8 => if self.latch0.get() != Latch::FE { self.latch0.set(Latch::FE) },
            0x1FD8..=0x1FDF => if self.latch1.get() != Latch::FD { self.latch1.set(Latch::FD) },
            0x1FE8..=0x1FEF => if self.latch1.get() != Latch::FE { self.latch1.set(Latch::FE) },
            _ => {}
        }
    }
}

impl Memory for Mmc2ChrRom {

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {

        let index = match addr {
            0x0000..=0x0FFF => self.memory_bank_map_0x0000[self.latch0.get().index()],
            0x1000..=0x1FFF => self.memory_bank_map_0x1000[self.latch1.get().index()],
            _ => panic!("invalid address: 0x{:04X}", addr),
        };

        let remapped_addr = addr & 0x0FFF;
        let byte = self.memory_banks[index].read_byte(remapped_addr);
        self.update_latch_on_read(addr);

        byte
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl BusDevice for Mmc2ChrRom {
    fn get_name(&self) -> String {
        format!("{} (chr_rom)", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::WRAM(MemoryType::Mmc2SwitchableMemory)
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_ADDRESS_SPACE
    }
}


#[derive(Debug)]
pub struct Mmc2Cartridge {
    prg_rom: Mmc2PrgRom,
    prg_ram: Rc<RefCell<MemoryBank>>,
    chr_rom: Rc<RefCell<Mmc2ChrRom>>,
    device_type: BusDeviceType,
    mirroring: Rc<RefCell<PpuNameTableMirroring>>,
    region: Region
}

impl Mmc2Cartridge {
    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize, chr_rom_offset: u64, chr_rom_size: usize,
               mirroring: PpuNameTableMirroring, region: Region) -> Result<Mmc2Cartridge, CartridgeError> {
        let prg_rom_memory_banks = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, MMC2_PRG_ROM_BANK_SIZE, PRG_ROM_ADDRESS_SPACE)?;
        let chr_memory_banks = cartridge::create_chr_memory(Some(&mut data), chr_rom_offset, chr_rom_size, MMC2_CHR_MEMORY_BANK_SIZE, true, PPU_ADDRESS_SPACE)?;
        let prg_ram_memory_bank = MemoryBank::new(MMC2_PRG_RAM_BANK_SIZE, PRG_RAM_ADDRESS_SPACE);

        info!("prg_rom: bank size: {}, number of banks: {}", prg_rom_size, prg_rom_memory_banks.len());
        info!("chr_rom: bank size: {}, number of banks: {}", chr_rom_size, chr_memory_banks.len());

        let cartridge = Mmc2Cartridge {
            prg_rom: Mmc2PrgRom::new(prg_rom_memory_banks)?,
            prg_ram: Rc::new(RefCell::new(prg_ram_memory_bank)),
            chr_rom: Rc::new(RefCell::new(Mmc2ChrRom::new(chr_memory_banks))),
            device_type: BusDeviceType::CARTRIDGE(MMC2),
            mirroring: Rc::new(RefCell::new(mirroring)),
            region,
        };


        Ok(cartridge)
    }
}

impl<'a> BusDevice for Mmc2Cartridge {
    fn get_name(&self) -> String {
        format!("{} (prg_rom)", MAPPER_NAME)
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_virtual_address_range(&self) -> (u16, u16) {
        (PRG_ROM_ADDRESS_SPACE.0, PRG_ROM_ADDRESS_SPACE.1)
    }
}

impl<'a> Memory for Mmc2Cartridge {
    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.prg_rom.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        match addr {
            0x0000..=0x1FFF => {},
            0x2000..=0x2FFF => self.prg_rom.memory_bank_map[0] = (value & 0x0F) as usize,
            0x3000..=0x3FFF => self.chr_rom.borrow_mut().memory_bank_map_0x0000[Latch::FD.index()] = (value & 0x1F) as usize,
            0x4000..=0x4FFF => self.chr_rom.borrow_mut().memory_bank_map_0x0000[Latch::FE.index()] = (value & 0x1F) as usize,
            0x5000..=0x5FFF => self.chr_rom.borrow_mut().memory_bank_map_0x1000[Latch::FD.index()] = (value & 0x1F) as usize,
            0x6000..=0x6FFF => self.chr_rom.borrow_mut().memory_bank_map_0x1000[Latch::FE.index()] = (value & 0x1F) as usize,
            0x7000..=0x7FFF => {
                match value & 0x01 {
                    0 => *self.mirroring.borrow_mut() = PpuNameTableMirroring::Vertical,
                    1 => *self.mirroring.borrow_mut() = PpuNameTableMirroring::Horizontal,
                    _ => unreachable!(),
                }
            },
            _ => panic!("invalid address: 0x{:04X}", addr),
        }

        Ok(())
    }

    fn size(&self) -> usize {
        self.prg_rom.size()
    }
}

impl<'a> Cartridge for Mmc2Cartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_rom.clone()
    }

    fn get_prg_ram(&self) -> Option<Rc<RefCell<dyn BusDevice>>> {
        Some(self.prg_ram.clone())
    }

    fn get_mirroring(&self) -> Rc<RefCell<PpuNameTableMirroring>> {
        self.mirroring.clone()
    }

    fn get_region(&self) -> Region {
        self.region
    }
}

impl FromINes for Mmc2Cartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<Mmc2Cartridge, LoaderError>
    where
        Self: Sized
    {
        let reader = BufReader::new(file);

        let cartridge = Mmc2Cartridge::new(reader,
                                             header.prg_offset(), header.prg_rom_size,
                                             header.chr_offset().unwrap_or(0), header.chr_rom_size,
                                             header.nametables_layout, header.region)?;

        Ok(cartridge)
    }
}