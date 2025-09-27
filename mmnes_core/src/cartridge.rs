use std::cell::RefCell;
use std::fmt;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::io::{BufReader, Error, Read, Seek, SeekFrom};
use std::rc::Rc;
use log::debug;
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::memory_ciram::PpuNameTableMirroring;

pub const PPU_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x1FFF);
pub const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);

#[derive(Debug, PartialEq)]
pub enum CartridgeError {
    LoadingError(String),
    MemoryError(MemoryError),
    Unsupported(String),
    IllegalState(String)
}

impl From<Error> for CartridgeError {
    fn from(error: Error) -> Self {
        CartridgeError::LoadingError(error.to_string())
    }
}

impl From<MemoryError> for CartridgeError {
    fn from(error: MemoryError) -> Self {
        CartridgeError::MemoryError(error)
    }
}

impl Display for CartridgeError {
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        match self {
            CartridgeError::LoadingError(s) => { write!(f, "loading error: {}", s) }
            CartridgeError::MemoryError(e) => { write!(f, "-> memory error: {}", e) }
            CartridgeError::Unsupported(s) => { write!(f, "unsupported: {}", s) },
            CartridgeError::IllegalState(s) => { write!(f, "illegal state: {}", s) }
        }
    }
}

#[derive(Default, Debug, Clone)]
pub enum CartridgeType {
    #[default]
    NESCARTRIDGE,
    NROM,
    UNROM,
    MMC1
}

impl Display for CartridgeType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            CartridgeType::NESCARTRIDGE => { write!(f, "cartridge type: NESCARTRIDGE") },
            CartridgeType::NROM => { write!(f, "cartridge type: NROM") }
            CartridgeType::UNROM => { write!(f, "cartridge type: UNROM") }
            CartridgeType::MMC1 => { write!(f, "cartridge type: MMC1") }
        }
    }
}

impl PartialEq for CartridgeType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (CartridgeType::NESCARTRIDGE, CartridgeType::NESCARTRIDGE) => true,
            (CartridgeType::NROM, CartridgeType::NROM) => true,
            _ => false,
        }
    }
}

pub trait Cartridge: BusDevice {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>>;
    fn get_prg_ram(&self) -> Option<Rc<RefCell<dyn BusDevice>>> {
        None
    }
    fn get_mirroring(&self) -> Rc<RefCell<PpuNameTableMirroring>>;
}

/***
 * helper functions
 ***/

pub fn write_rom_data(rom: &mut dyn Memory, size: usize, data: &mut BufReader<File>) -> Result<(), CartridgeError> {
    let mut buf = vec![0u8; size];
    data.read_exact(&mut buf)?;

    for (i, &byte) in buf.iter().enumerate() {
        rom.write_byte(i as u16, byte)?;
    }

    Ok(())
}

pub fn get_chr_memory_size_and_type(chr_rom_size: usize, chr_ram_size: usize) -> (usize, bool) {
    /***
     * loader guarantees that one of chr_rom and chr_ram is non-zero.
     */
    if chr_rom_size > 0 {
        (chr_rom_size, true)
    } else {
        (chr_ram_size, false)
    }
}

fn memory_banks_vec(total_size: usize, bank_size: usize) -> Result<(Vec<MemoryBank>, usize), CartridgeError> {
    let num_memory_banks = total_size / bank_size;
    let memory_banks: Vec<MemoryBank> = Vec::with_capacity(num_memory_banks);

    Ok((memory_banks, num_memory_banks))
}

pub fn create_split_ram_memory(total_size: usize, bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    let (mut memory_banks, num_memory_banks) = memory_banks_vec(total_size, bank_size)?;

    for _ in 0..num_memory_banks {
        let ram = MemoryBank::new(bank_size, address_range);
        memory_banks.push(ram);
    }

    Ok(memory_banks)
}

pub fn create_split_rom_memory(data: &mut BufReader<File>, offset: u64, total_size: usize, bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    let (mut memory_banks, num_memory_banks) = memory_banks_vec(total_size, bank_size)?;

    data.seek(SeekFrom::Start(offset))?;

    for bank in 0..num_memory_banks {
        debug!("CARTRIDGE: loading rom data ({} / {} KB) in memory bank {} / {} (id: {}), offset: 0x{:04X}...",
                bank_size * (bank + 1), total_size, bank + 1, num_memory_banks, bank, data.stream_position()?);

        let mut rom = MemoryBank::new(bank_size, address_range);
        write_rom_data(&mut rom, bank_size, data)?;

        memory_banks.push(rom);
    }

    Ok(memory_banks)
}

pub fn create_chr_rom_memory(data: &mut BufReader<File>, chr_rom_offset: u64, chr_rom_total_size: usize, chr_rom_bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    create_split_rom_memory(data, chr_rom_offset, chr_rom_total_size, chr_rom_bank_size, address_range)
}

pub fn create_chr_ram_memory(chr_ram_total_size: usize, chr_ram_bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    create_split_ram_memory(chr_ram_total_size, chr_ram_bank_size, address_range)
}

pub fn create_chr_memory(data: Option<&mut BufReader<File>>, offset: u64, total_size: usize, bank_size: usize, is_chr_rom: bool, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    let chr = if is_chr_rom {
        if let Some(mut data) = data {
            create_chr_rom_memory(&mut data, offset, total_size, bank_size, address_range)?
        } else {
            Err(CartridgeError::IllegalState(format!("data can not be empty for CHR rom (offset: 0x{:04X})", offset)))?
        }
    } else {
        create_chr_ram_memory(total_size, bank_size, address_range)?
    };

    Ok(chr)
}

pub fn create_prg_rom_memory(data: &mut BufReader<File>, prg_rom_offset: u64, prg_rom_total_size: usize, prg_rom_bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    create_split_rom_memory(data, prg_rom_offset, prg_rom_total_size, prg_rom_bank_size, address_range)
}

pub fn create_prg_ram_memory(prg_ram_total_size: usize, prg_bank_size: usize, address_range: (u16, u16)) -> Result<Vec<MemoryBank>, CartridgeError> {
    create_split_ram_memory(prg_ram_total_size, prg_bank_size, address_range)
}

pub fn get_first_bank_or_fail(mut memory_banks: Vec<MemoryBank>, total_size: usize, bank_size: usize, is_rom: bool) -> Result<MemoryBank, CartridgeError> {
    let len = memory_banks.len();

    if let Some(bank) = memory_banks.pop() && len == 1 {
        Ok(bank)
    } else {
        Err(CartridgeError::LoadingError(
            format!("unexpected number of banks (expected: 1, found: {}), total size: {}, bank size: {}, number of banks: {}, is ram: {}",
                    len, total_size, bank_size, len, !is_rom)
        ))
    }
}