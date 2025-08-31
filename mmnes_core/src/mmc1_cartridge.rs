use std::cell::RefCell;
use std::fmt::Display;
use std::fs::File;
use std::io::BufReader;
use std::rc::Rc;
use log::{debug, info};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge;
use crate::cartridge::{Cartridge, CartridgeError, PPU_ADDRESS_SPACE};
use crate::cartridge::CartridgeType::MMC1;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError, MemoryType};
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
 * additional specifications: https://www.raphnet.net/electronique/nes_cart/mmc1.txt
 *
 * XXX NOT IMPLEMENTED:
 *  - Consecutive writes that are too close together are ignored.
 *
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
    PrgBankMode32k,     // 0, 1: switch 32 KB at $8000, ignoring low bit of bank number
    PrgBankMode16kHi,   // 2: fix first bank at $8000 and switch 16 KB bank at $C000
    PrgBankMode16kLo,   // 3: fix last bank at $C000 and switch 16 KB bank at $8000
    ChrBankMode8k,      // 0: switch 8 KB at a time
    ChrBankMode4k       // 1: switch two separate 4 KB banks
}

impl Display for SwitchingMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SwitchingMode::PrgBankMode32k => write!(f, "32k PRG-ROM bank mode"),
            SwitchingMode::PrgBankMode16kHi => write!(f, "16k PRG-ROM bank mode (hi)"),
            SwitchingMode::PrgBankMode16kLo => write!(f, "16k PRG-ROM bank mode (lo)"),
            SwitchingMode::ChrBankMode8k => write!(f, "8k CHR-ROM bank mode"),
            SwitchingMode::ChrBankMode4k => write!(f, "4k CHR-ROM bank mode"),
        }
    }
}

#[derive(Debug)]
struct SwitchableMemory {
    name: String,
    size: usize,
    memory_banks: Vec<MemoryBank>,
    num_memory_banks: usize,
    current_bank_lo: usize,
    current_bank_hi: usize,
    addr_half_lo: (u16, u16),
    addr_half_hi: (u16, u16),
}

impl SwitchableMemory {
    fn get_memory_name(&self) -> &String {
        &self.name
    }

    fn get_current_bank_index_and_effective_addr(&self, addr: u16) -> Result<(usize, u16), MemoryError> {
        match addr {
            x if x >= self.addr_half_lo.0 && x <= self.addr_half_lo.1 => {
                Ok((self.current_bank_lo, addr - self.addr_half_lo.0))
            },
            x if x >= self.addr_half_hi.0 && x <= self.addr_half_hi.1 => {
                Ok((self.current_bank_hi, addr - self.addr_half_hi.0))
            },
            _ => Err(MemoryError::OutOfRange(addr))
        }
    }


    fn get_bank_by_address(&self, addr: u16) -> Result<(&MemoryBank, u16), MemoryError> {
        let (bank_index, effective_addr) = self.get_current_bank_index_and_effective_addr(addr)?;
        let bank = &self.memory_banks[bank_index];

        Ok((bank, effective_addr))
    }

    fn get_bank_by_address_as_mut(&mut self, addr: u16) -> Result<(&mut MemoryBank, u16), MemoryError> {
        let (bank_index, effective_addr) = self.get_current_bank_index_and_effective_addr(addr)?;
        let bank = &mut self.memory_banks[bank_index];

        Ok((bank, effective_addr))
    }
}

impl Memory for SwitchableMemory {

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (bank, effective_addr) = self.get_bank_by_address(addr)?;
        bank.read_byte(effective_addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let (bank, effective_addr): (&mut MemoryBank, u16) = self.get_bank_by_address_as_mut(addr)?;
        bank.write_byte(effective_addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let (bank, effective_addr) = self.get_bank_by_address(addr)?;
        bank.read_word(effective_addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let (bank, effective_addr): (&mut MemoryBank, u16) = self.get_bank_by_address_as_mut(addr)?;
        bank.write_word(effective_addr, value)
    }

    fn size(&self) -> usize {
        self.size
    }
}

impl BusDevice for SwitchableMemory {
    fn get_name(&self) -> String {
        format!("{} ({})", MAPPER_NAME, self.get_memory_name())
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::WRAM(MemoryType::SwitchableMemory)
    }

    fn get_address_range(&self) -> (u16, u16) {
        (self.addr_half_lo.0, self.addr_half_hi.1)
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        self.addr_half_lo.0 <= addr && addr <= self.addr_half_hi.1
    }
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
    chr_rom: Rc<RefCell<SwitchableMemory>>,
    device_type: BusDeviceType,
    mirroring: PpuNameTableMirroring
}

impl Mmc1Cartridge {

    fn reset_shift_register(&mut self) {
        self.shift_register = 0x10;
    }

    fn reset(&mut self) -> Result<(), MemoryError> {
        self.reset_shift_register();

        self.control_register |= 0x0C;
        self.apply_control()?;

        Ok(())
    }

    fn control_nametable_mirroring(&mut self) -> Result<(), MemoryError> {
        match self.control_register & 0x03 {
            0 => { self.mirroring = PpuNameTableMirroring::SingleScreen; },
            1 => { self.mirroring = PpuNameTableMirroring::SingleScreen; },
            2 => { self.mirroring = PpuNameTableMirroring::Vertical; },
            3 => { self.mirroring = PpuNameTableMirroring::Horizontal; },
            _ => unreachable!(),
        }

        //info!("MMC1: nametable mirroring: {:?}", self.mirroring);
        Ok(())
    }

    fn get_prg_bank_index_from_prg_register(&self) -> usize {
        let index = if self.prg_rom_bank_mode == SwitchingMode::PrgBankMode32k {
            let original = (self.control_prg_bank & 0x0E) as usize;
            original * 2
        } else {
            (self.control_prg_bank & 0x0F) as usize
        };

        index
    }

    fn get_prg_bank_indexes_16k_hi(&self) -> (usize, usize) {
        let bank_hi = self.get_prg_bank_index_from_prg_register();
        (0, bank_hi)
        //(0, self.prg_rom.current_bank_lo) https://www.raphnet.net/electronique/nes_cart/mmc1.txt
    }

    fn get_prg_bank_indexes_16k_lo(&self) -> (usize, usize) {
        let bank_lo = self.get_prg_bank_index_from_prg_register();
        let bank_hi = self.prg_rom.num_memory_banks - 1;
        (bank_lo, bank_hi)
        // (self.prg_rom.current_bank_hi, self.prg_rom.num_memory_banks - 1) https://www.raphnet.net/electronique/nes_cart/mmc1.txt
    }

    fn get_prg_bank_indexes_32k(&mut self) -> (usize, usize) {
        let bank_lo = self.get_prg_bank_index_from_prg_register();
        let bank_hi = bank_lo + 1;

        (bank_lo, bank_hi)
    }

    fn remap_prg_banks(&mut self, bank_lo: usize, bank_hi: usize) -> Result<(), MemoryError> {
        let previous_bank_lo = self.prg_rom.current_bank_lo;
        let previous_bank_hi = self.prg_rom.current_bank_hi;

        self.prg_rom.current_bank_lo = bank_lo;
        self.prg_rom.current_bank_hi = bank_hi;

        info!("MMC1: switched prg rom banks: low {} -> {}, high {} -> {}, mode: {}",
            previous_bank_lo, self.prg_rom.current_bank_lo, previous_bank_hi, self.prg_rom.current_bank_hi, self.prg_rom_bank_mode);

        Ok(())
    }

    fn get_chr_bank_indexes_4k(&self) -> (usize, usize) {
        let chr = self.chr_rom.borrow();
        let bank_lo = chr.current_bank_lo;
        let bank_hi = chr.current_bank_hi;

        (bank_lo, bank_hi)
    }

    fn get_chr_bank_indexes_8k(&self) -> (usize, usize) {
        let bank_lo = (self.control_chr_bank0 & 0xFE) as usize;
        let bank_hi = bank_lo + 1;

        (bank_lo, bank_hi)
    }

    fn remap_chr_banks(&mut self, bank_lo: usize, bank_hi: usize) -> Result<(), MemoryError> {
        let previous_bank_lo = self.chr_rom.borrow_mut().current_bank_lo;
        let previous_bank_hi = self.chr_rom.borrow_mut().current_bank_hi;

        self.chr_rom.borrow_mut().current_bank_lo = bank_lo;
        self.chr_rom.borrow_mut().current_bank_hi = bank_hi;

        info!("MMC1: switched chr rom banks: low {} -> {}, high {} -> {}, mode: {}",
            previous_bank_lo, self.chr_rom.borrow().current_bank_lo, previous_bank_hi, self.chr_rom.borrow().current_bank_hi, self.chr_rom_bank_mode);

        Ok(())
    }

    fn control_prg_rom_mode(&mut self) -> Result<(), MemoryError> {
        let (bank_lo, bank_hi) = match (self.control_register >> 2) & 0x03 {
            0 | 1 => {  // PrgBankMode32k
                if self.prg_rom_bank_mode != SwitchingMode::PrgBankMode32k {
                    self.prg_rom_bank_mode = SwitchingMode::PrgBankMode32k;
                    self.get_prg_bank_indexes_32k()
                } else {
                    (self.prg_rom.current_bank_lo, self.prg_rom.current_bank_hi)
                }
            },
            2 => {  // PrgBankMode16kHi
                if self.prg_rom_bank_mode != SwitchingMode::PrgBankMode16kHi {
                    self.prg_rom_bank_mode = SwitchingMode::PrgBankMode16kHi;
                    self.get_prg_bank_indexes_16k_hi()
                } else {
                    (self.prg_rom.current_bank_lo, self.prg_rom.current_bank_hi)
                }
            },
            3 => {  // PrgBankMode16kLo
                if self.prg_rom_bank_mode != SwitchingMode::PrgBankMode16kLo {
                    self.prg_rom_bank_mode = SwitchingMode::PrgBankMode16kLo;
                    self.get_prg_bank_indexes_16k_lo()
                } else {
                    (self.prg_rom.current_bank_lo, self.prg_rom.current_bank_hi)
                }
            },
            _ => unreachable!(),
        };

        self.remap_prg_banks(bank_lo, bank_hi)?;
        Ok(())
    }

    fn control_chr_rom_mode(&mut self) -> Result<(), MemoryError> {
        let (bank_lo, bank_hi) = match (self.control_register >> 4) & 0x01 {
            0 => {
                if self.chr_rom_bank_mode != SwitchingMode::ChrBankMode8k {
                    self.get_chr_bank_indexes_8k()
                } else {
                    (self.chr_rom.borrow().current_bank_lo, self.chr_rom.borrow().current_bank_hi)
                }
            },
            1 => {
                if self.chr_rom_bank_mode != SwitchingMode::ChrBankMode4k {
                    self.get_chr_bank_indexes_4k()
                } else {
                    (self.chr_rom.borrow().current_bank_lo, self.chr_rom.borrow().current_bank_hi)
                }
            },
            _ => unreachable!(),
        };

        self.remap_chr_banks(bank_lo, bank_hi)?;
        Ok(())
    }

    fn apply_control(&mut self) -> Result<(), MemoryError> {

        //info!("MMC1: applying control register: 0x{:02X}", self.control_register);
        self.control_nametable_mirroring()?;
        self.control_prg_rom_mode()?;
        self.control_chr_rom_mode()?;

        Ok(())
    }

    fn map_shift_register_to_control_register(&mut self) -> Result<(), MemoryError> {
        self.control_register = self.shift_register & 0x1F;
        self.apply_control()?;

        Ok(())
    }

    fn map_shift_register_to_chr0_register(&mut self) -> Result<(), MemoryError> {
        self.control_chr_bank0 = self.shift_register & 0x1F;

        //info!("MMC1: chr bank low: {:?}", self.chr_rom_bank_mode);
        Ok(())
    }

    fn map_shift_register_to_chr1_register(&mut self) -> Result<(), MemoryError> {
        self.control_chr_bank1 = self.shift_register & 0x1F;

        //info!("MMC1: chr bank high: {:?}", self.chr_rom_bank_mode);
        Ok(())
    }

    fn map_shift_register_to_prg_register(&mut self) -> Result<(), MemoryError> {
        self.control_prg_bank = self.shift_register & 0x1F;

        let (bank_lo, bank_hi) = match self.prg_rom_bank_mode {
            SwitchingMode::PrgBankMode32k => { self.get_prg_bank_indexes_32k() },
            SwitchingMode::PrgBankMode16kHi => { self.get_prg_bank_indexes_16k_hi() },
            SwitchingMode::PrgBankMode16kLo => { self.get_prg_bank_indexes_16k_lo() },
            _ => unreachable!(),
        };

        self.remap_prg_banks(bank_lo, bank_hi)?;
        Ok(())
    }

    /***
     * The MMC1 copies bit 0 and the shift register contents into an internal register
     * selected by bits 14 and 13 of the address, and then it clears the shift register
     ***/
    fn write_shift_register_to_internal_register(&mut self, addr: u16) -> Result<(), MemoryError> {
        match (addr & 0x6000) >> 13  {
            0 => { self.map_shift_register_to_control_register()?; },     // register 0 (control):    0x8000 - 0x9FFF
            1 => { self.map_shift_register_to_chr0_register()?; },        // register 1 (chr bank 0): 0xA000 - 0xBFFF
            2 => { self.map_shift_register_to_chr1_register()?; },        // register 2 (chr bank 1): 0xC000 - 0xDFFF
            3 => { self.map_shift_register_to_prg_register()?; },         // register 3 (prg bank):   0xE000 - 0xFFFF
            _ => unreachable!(),
        }

        //info!("MMC1: mapped shift register to internal register at 0x{:04X}", addr);
        self.reset_shift_register();
        Ok(())
    }

    fn build_switchable_memory(name: String, size: usize, memory_banks: Vec<MemoryBank>) -> Result<SwitchableMemory, MemoryError> {
        let num_memory_banks = memory_banks.len();

        let memory = SwitchableMemory {
            name,
            size,
            memory_banks,
            num_memory_banks,
            current_bank_lo: 0,
            current_bank_hi: if num_memory_banks == 0 { 0 } else { num_memory_banks - 1 },
            addr_half_lo: (0, ((size / 2) - 1) as u16),
            addr_half_hi: ((size / 2) as u16, (size - 1) as u16)
        };

        //info!("built switchable_memory: {}, addr_half_lo: 0x{:04X} - 0x{:04X}, addr_half_hi: 0x{:04X} - 0x{:04X}",
        //    memory.name, memory.addr_half_lo.0, memory.addr_half_lo.1, memory.addr_half_hi.0, memory.addr_half_hi.1);

        Ok(memory)
    }

    pub fn new(mut data: BufReader<File>,
               prg_rom_offset: u64, prg_rom_size: usize, prg_ram_size: usize,
               chr_rom_offset: u64, chr_rom_size: usize, chr_ram_size: usize,
               mirroring: PpuNameTableMirroring) -> Result<Mmc1Cartridge, CartridgeError> {

        let prg_rom_memory_banks = cartridge::create_prg_rom_memory(&mut data, prg_rom_offset, prg_rom_size, MMC1_PRG_ROM_BANK_SIZE, PRG_ROM_ADDRESS_SPACE)?;
        let prg_rom_addr_size = (PRG_ROM_ADDRESS_SPACE.1 - PRG_ROM_ADDRESS_SPACE.0 + 1) as usize;

        let (chr_memory_size, is_chr_rom) = cartridge::get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);
        let rom_data = if is_chr_rom { Some(&mut data) } else { None };

        let chr_memory_banks = cartridge::create_chr_memory(rom_data, chr_rom_offset, chr_memory_size, MMC1_CHR_MEMORY_BANK_SIZE, is_chr_rom, PPU_ADDRESS_SPACE)?;
        let chr_addr_size = (PPU_ADDRESS_SPACE.1 - PPU_ADDRESS_SPACE.0 + 1) as usize;

        let prg_ram_memory_banks = if prg_ram_size > 0 {
            cartridge::create_prg_ram_memory(prg_ram_size, MMC1_PRG_RAM_BANK_SIZE, PRG_RAM_ADDRESS_SPACE)?
        } else {
            Vec::new()
        };
        let prg_ram_addr_size = (PRG_RAM_ADDRESS_SPACE.1 - PRG_RAM_ADDRESS_SPACE.0 + 1) as usize;

        let mut cartridge = Mmc1Cartridge {
            shift_register: 0x10,
            control_register: 0x0C,
            control_chr_bank0: 0,
            control_chr_bank1: 0,
            control_prg_bank: 0,
            prg_rom_bank_mode: SwitchingMode::PrgBankMode16kLo,
            chr_rom_bank_mode: SwitchingMode::ChrBankMode8k,
            prg_rom: Mmc1Cartridge::build_switchable_memory("prg_rom".to_string(), prg_rom_addr_size, prg_rom_memory_banks)?,
            prg_ram: Mmc1Cartridge::build_switchable_memory("prg_ram".to_string(), prg_ram_addr_size, prg_ram_memory_banks)?,
            chr_rom: Rc::new(RefCell::new(Mmc1Cartridge::build_switchable_memory("chr_rom".to_string(), chr_addr_size, chr_memory_banks)?)),
            device_type: BusDeviceType::CARTRIDGE(MMC1),
            mirroring,
        };

        cartridge.apply_control()?;
        Ok(cartridge)
    }


    fn build(file: File,
             prg_rom_offset: u64, prg_rom_size: usize, prg_ram_size: usize,
             chr_rom_offset: Option<u64>, chr_rom_size: usize, chr_ram_size: usize, mirroring: PpuNameTableMirroring) -> Result<Mmc1Cartridge, LoaderError> {
        debug!("creating MMC1 cartridge");

        let reader = BufReader::new(file);
        let chr_rom_offset = if let Some(chr_rom_offset_unwrapped) = chr_rom_offset { chr_rom_offset_unwrapped } else { 0 };

        let cartridge = Mmc1Cartridge::new(reader, prg_rom_offset, prg_rom_size, prg_ram_size, chr_rom_offset, chr_rom_size, chr_ram_size, mirroring)?;
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

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.prg_rom.read_byte(addr)
    }

    /***
     * On the fifth write, the MMC1 copies bit 0 and the shift register contents into an internal register selected by bits 14 and 13 of the address, and then it clears the shift register.
     * Only on the fifth write does the address matter, and even then, only bits 14 and 13 of the address matter because the mapper doesn't see the lower address bits
     * (similar to the mirroring seen with PPU registers). After the fifth write, the shift register is cleared automatically.
     ***/
    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        if value & 0x80 != 0 {
            self.reset()?; // XXX PAS SUR DU TOUT DU TOUT
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
        self.prg_rom.read_word(addr)
    }

    fn size(&self) -> usize {
        self.prg_rom.size
    }
}

impl Cartridge for Mmc1Cartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        self.chr_rom.clone()
    }

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        self.mirroring.clone()
    }
}