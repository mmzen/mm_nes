use std::cell::RefCell;
use std::fs::File;
use std::rc::Rc;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::ines_loader::{FromINes, INesRomHeader};
use crate::loader::LoaderError;
use crate::memory::{Memory, MemoryError};
use crate::ppu::PpuNameTableMirroring;

#[derive(Debug)]
pub struct Mmc1Cartridge {}

impl Mmc1Cartridge {}

impl Cartridge for Mmc1Cartridge {
    fn get_chr_rom(&self) -> Rc<RefCell<dyn BusDevice>> {
        todo!()
    }

    fn get_mirroring(&self) -> PpuNameTableMirroring {
        todo!()
    }
}

impl BusDevice for Mmc1Cartridge {
    fn get_name(&self) -> String {
        todo!()
    }

    fn get_device_type(&self) -> BusDeviceType {
        todo!()
    }

    fn get_address_range(&self) -> (u16, u16) {
        todo!()
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        todo!()
    }
}

impl Memory for Mmc1Cartridge {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        todo!()
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        todo!()
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        todo!()
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        todo!()
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        todo!()
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        todo!()
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        todo!()
    }
}

impl FromINes for Mmc1Cartridge {
    #[allow(refining_impl_trait)]
    fn from_ines(file: File, header: INesRomHeader) -> Result<Mmc1Cartridge, LoaderError>
    where
        Self: Sized
    {
        //let cartridge = Mmc1Cartridge::build(file,
        //                                      header.prg_offset(), header.prg_rom_size,
        //                                      header.chr_offset(), header.chr_rom_size,
        //                                      header.chr_ram_size, header.nametables_layout)?;

        let cartridge = Mmc1Cartridge {}; // Placeholder for actual implementation
        Ok(cartridge)
    }
}