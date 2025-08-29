use std::cell::RefCell;
use std::fs::File;
use std::io::BufReader;
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

const PRG_ROM_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);
const PRG_RAM_ADDRESS_SPACE: (u16, u16) = (0x6000, 0x7FFF);
const MMC1_PRG_ROM_BANK_SIZE: usize = 16 * 1024;
const MMC1_PRG_RAM_BANK_SIZE: usize = 8 * 1024;
const MMC1_CHR_MEMORY_BANK_SIZE: usize = 4 * 1024;
const MAPPER_NAME: &str = "MMC1";

/***
 * https://www.nesdev.org/wiki/MMC1
 * SxROM board types
 *
 * XXX NOT IMPLEMENTED:
 *  - Consecutive writes that are too close together are ignored.
 ***/


/***
 *   4bit0
 *   -----
 *   CPPMM
 *   |||||
 *   |||++- Nametable arrangement: (0: one-screen, lower bank; 1: one-screen, upper bank;
 *   |||               2: horizontal arrangement ("vertical mirroring", PPU A10);
 *   |||               3: vertical arrangement ("horizontal mirroring", PPU A11) )
 *   |++--- PRG-ROM bank mode (0, 1: switch 32 KB at $8000, ignoring low bit of bank number;
 *    |                         2: fix first bank at $8000 and switch 16 KB bank at $C000;
 *    |                         3: fix last bank at $C000 and switch 16 KB bank at $8000)
 *   +----- CHR-ROM bank mode (0: switch 8 KB at a time; 1: switch two separate 4 KB banks)
 ***/
#[derive(Debug, PartialEq)]
enum SwitchingMode {
    PrgBankMode0,  // 0, 1: switch 32 KB at $8000, ignoring low bit of bank number
    PrgBankMode2,  // 2: fix first bank at $8000 and switch 16 KB bank at $C000
    PrgBankMode3,  // 3: fix last bank at $C000 and switch 16 KB bank at $8000
    ChrBankMode0,  // 0: switch 8 KB at a time
    ChrBankMode1   // 1: switch two separate 4 KB banks
}

#[derive(Debug)]
struct SwitchableMemory {
    size: usize,
    memory_banks: Vec<MemoryBank>,
    num_memory_banks: usize,
    current_bank0: usize,  // 0x8000-0xBFFF
    current_bank1: usize,  // 0xC000-0xFFFF
}

#[derive(Debug)]
pub struct Mmc1Cartridge {
    shift_register: u8,
    control_register: u8,
    control_chr_bank0: u8,
    control_chr_bank1: u8,
    control_prg_bank: u8,
    prg_rom_bank_mode: SwitchingMode,
    chr_rom_bank_mode: SwitchingMode,
    prg_rom: SwitchableMemory,
    prg_ram: SwitchableMemory,
    chr_rom: SwitchableMemory,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring
}

impl Mmc1Cartridge {

    fn reset_shift_register(&mut self) {
        self.shift_register = 0x10;
    }

    fn reset(&mut self) -> Result<(), MemoryError> {
        self.reset_shift_register();
        self.prg_rom_bank_mode = SwitchingMode::PrgBankMode3;

        Ok(())
    }

    fn control_nametable_mirroring(&mut self) -> Result<(), MemoryError> {
        match self.control_register & 0x03 {
            0 => { self.mirroring = PpuNameTableMirroring::Single; },
            1 => { self.mirroring = PpuNameTableMirroring::Single; },
            2 => { self.mirroring = PpuNameTableMirroring::Vertical; },
            3 => { self.mirroring = PpuNameTableMirroring::Horizontal; },
            _ => unreachable!(),
        }

        Ok(())
    }

    fn control_prg_rom_mode(&mut self) -> Result<(), MemoryError> {
        match (self.control_register >> 2) & 0x03 {
            0 | 1 => { self.prg_rom_bank_mode = SwitchingMode::PrgBankMode0; },
            2 => { self.prg_rom_bank_mode = SwitchingMode::PrgBankMode2; },
            3 => { self.prg_rom_bank_mode = SwitchingMode::PrgBankMode3; },
            _ => unreachable!(),
        }

        Ok(())
    }

    fn control_chr_rom_mode(&mut self) -> Result<(), MemoryError> {
        match (self.control_register >> 4) & 0x01 {
            0 => { self.chr_rom_bank_mode = SwitchingMode::ChrBankMode0; },
            1 => { self.chr_rom_bank_mode = SwitchingMode::ChrBankMode1; },
            _ => unreachable!(),
        }

        Ok(())
    }

    fn write_shift_register_to_control_register(&mut self) -> Result<(), MemoryError> {
        self.control_register = self.shift_register;

        self.control_nametable_mirroring()?;
        self.control_prg_rom_mode()?;
        self.control_chr_rom_mode()?;

        Ok(())
    }

    fn write_shift_register_to_chr0_register(&mut self) -> Result<(), MemoryError> {
        self.control_chr_bank0 = self.shift_register;

        let selector = if self.chr_rom_bank_mode == SwitchingMode::ChrBankMode0 {
            self.control_chr_bank0 & 0xFE
        } else {
            self.control_chr_bank0
        } as usize;

        self.chr_rom.current_bank0 = selector;

        Ok(())
    }

    fn write_shift_register_to_chr1_register(&mut self) -> Result<(), MemoryError> {
        self.control_chr_bank1 = self.shift_register;

        if self.chr_rom_bank_mode != SwitchingMode::ChrBankMode0 {
            self.chr_rom.current_bank1 = self.control_chr_bank1 as usize;
        }

        Ok(())
    }

    fn write_shift_register_to_prg_register(&mut self) -> Result<(), MemoryError> {
        self.control_prg_bank = self.shift_register;

        match self.prg_rom_bank_mode {
            SwitchingMode::PrgBankMode0 => {
                self.prg_rom.current_bank0 = (self.control_prg_bank & 0xFE) as usize % self.prg_rom.num_memory_banks;
            },
            SwitchingMode::PrgBankMode2 => {
                self.prg_rom.current_bank1 = self.control_prg_bank as usize;
            },
            SwitchingMode::PrgBankMode3 => {
                self.prg_rom.current_bank0 = self.control_prg_bank as usize;
            }
            _ => unreachable!(),
        }

        Ok(())
    }

    /***
     * The MMC1 copies bit 0 and the shift register contents into an internal register
     * selected by bits 14 and 13 of the address, and then it clears the shift register
     ***/
    fn write_shift_register_to_internal_register(&mut self, addr: u16) -> Result<(), MemoryError> {
        match (addr & 0x6000) >> 13  {
            0 => { self.write_shift_register_to_control_register()?; },
            1 => { self.write_shift_register_to_chr0_register()?; },
            2 => { self.write_shift_register_to_chr1_register()?; },
            3 => { self.write_shift_register_to_prg_register()?; },
            _ => unreachable!(),
        }

        self.reset()?;
        Ok(())
    }

    fn read_prg_ram(&self, addr: u16) -> Result<u8, MemoryError> {
        self.prg_ram.memory_banks[self.prg_ram.current_bank0].read_byte(addr)
    }

    fn read_prg_rom(&self, addr: u16) -> Result<u8, MemoryError> {
        match self.prg_rom_bank_mode {
            SwitchingMode::PrgBankMode0 => {
                self.prg_rom.memory_banks[self.prg_rom.current_bank0].read_byte(addr)
            },
            SwitchingMode::PrgBankMode2 | SwitchingMode::PrgBankMode3 => {
                match addr {
                    0x8000..=0xBFFF => {
                        self.prg_rom.memory_banks[self.prg_rom.current_bank0].read_byte(addr)
                    },
                    0xC000..=0xFFFF => {
                        self.prg_rom.memory_banks[self.prg_rom.current_bank1].read_byte(addr)
                    },
                    _ => unreachable!()
                }
            },
            _ => unreachable!()
        }
    }

    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize, prg_ram_size: usize,
               chr_rom_offset: u64, chr_rom_size: usize, chr_ram_size: usize,
               mirroring: PpuNameTableMirroring) -> Result<Mmc1Cartridge, CartridgeError> {

        let (prg_rom_memory_banks, prg_rom_num_memory_banks) = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, MMC1_PRG_ROM_BANK_SIZE, PRG_ROM_ADDRESS_SPACE)?;

        let (chr_memory_size, is_chr_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);
        let rom_data = if is_chr_rom { Some(&mut data) } else { None };

        let (chr_memory_banks, num_chr_banks) = cartridge::create_chr_memory(rom_data, chr_rom_offset, chr_memory_size, MMC1_CHR_MEMORY_BANK_SIZE, is_chr_rom, PPU_ADDRESS_SPACE)?;

        let (prg_ram_memory_banks, prg_ram_num_memory_banks) = if prg_ram_size > 0 {
            cartridge::create_prg_ram_memory(prg_ram_size, MMC1_PRG_RAM_BANK_SIZE, PRG_RAM_ADDRESS_SPACE)?
        } else {
            (Vec::new(), 0)
        };

        let cartridge = Mmc1Cartridge {
            shift_register: 0x10,
            control_register: 0,
            control_chr_bank0: 0,
            control_chr_bank1: 0,
            control_prg_bank: 0,
            prg_rom_bank_mode: SwitchingMode::PrgBankMode3,
            chr_rom_bank_mode: SwitchingMode::PrgBankMode0,
            prg_rom: SwitchableMemory {
                size: prg_rom_size,
                memory_banks: prg_rom_memory_banks,
                num_memory_banks: prg_rom_num_memory_banks,
                current_bank0: 0,
                current_bank1: 0,
            },
            prg_ram: SwitchableMemory {
                size: prg_ram_size,
                memory_banks: prg_ram_memory_banks,
                num_memory_banks: prg_ram_num_memory_banks,
                current_bank0: 0,
                current_bank1: 0,
            },
            chr_rom: SwitchableMemory {
                size: chr_rom_size,
                memory_banks: chr_memory_banks,
                num_memory_banks: num_chr_banks,
                current_bank0: 0,
                current_bank1: 0,
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
        PRG_ROM_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        PRG_ROM_ADDRESS_SPACE.0 <= addr && addr <= PRG_ROM_ADDRESS_SPACE.1
    }
}

impl Memory for Mmc1Cartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        match addr {
            0x6000..=0x7FFF => {
                self.read_prg_ram(addr)?;
            },
            0x8000..=0xFFFF => {
                self.read_prg_rom(addr)?;
            },
            _ => unreachable!()
        }

        Ok(0)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    /***
     * On the fifth write, the MMC1 copies bit 0 and the shift register contents into an internal register selected by bits 14 and 13 of the address, and then it clears the shift register.
     * Only on the fifth write does the address matter, and even then, only bits 14 and 13 of the address matter because the mapper doesn't see the lower address bits
     * (similar to the mirroring seen with PPU registers). After the fifth write, the shift register is cleared automatically.
     ***/
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        if value & 0x80 != 0 {
            self.reset_shift_register();
        } else {
            let final_write = (self.shift_register & 0x01) == 1;
            self.shift_register = ((value & 0x01) << 4) | (self.shift_register >> 1);

            if final_write == true {
                self.write_shift_register_to_internal_register(addr)?;
            }
        }

        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        match self.prg_rom_bank_mode {
            SwitchingMode::PrgBankMode0 => {
                self.prg_rom.memory_banks[self.prg_rom.current_bank0].read_word(addr)
            },
            SwitchingMode::PrgBankMode2 | SwitchingMode::PrgBankMode3 => {
                match addr {
                    0x8000..=0xBFFF => {
                        self.prg_rom.memory_banks[self.prg_rom.current_bank0].read_word(addr)
                    },
                    0xC000..=0xFFFF => {
                        self.prg_rom.memory_banks[self.prg_rom.current_bank1].read_word(addr)
                    },
                    _ => unreachable!()
                }
            },
            _ => unreachable!()
        }
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
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