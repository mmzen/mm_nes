use std::cmp::Ordering;
use std::fmt;
use std::fmt::{Display, Formatter};
#[cfg(test)]
use mockall::mock;
use crate::apu::APUType;
use crate::cartridge::CartridgeType;
use crate::memory::{Memory, MemoryType};
#[cfg(test)]
use crate::memory::{MemoryError};
use crate::ppu::PPUType;

#[derive(Debug, Clone)]
pub enum BusDeviceType {
    WRAM(MemoryType),
    PPU(PPUType),
    APU(APUType),
    CARTRIDGE(CartridgeType),
}

impl Display for BusDeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BusDeviceType::WRAM(memory) => write!(f, "device type: WRAM - {}", memory),
            BusDeviceType::PPU(ppu) => write!(f, "device type: PPU - {}", ppu),
            BusDeviceType::APU(apu) => write!(f, "device type: APU - {}", apu),
            BusDeviceType::CARTRIDGE(cartridge) => write!(f, "device type: APU - {}", cartridge)
        }
    }
}

pub trait BusDevice: Memory {
    fn get_name(&self) -> String;
    fn get_device_type(&self) -> BusDeviceType;
    fn get_address_range(&self) -> (u16, u16);
    fn is_addr_in_boundary(&self, addr: u16) -> bool;
}

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub BusDeviceStub {}

    impl BusDevice for BusDeviceStub {
        fn get_name(&self) -> String;
        fn get_device_type(&self) -> BusDeviceType;
        fn get_address_range(&self) -> (u16, u16);
        fn is_addr_in_boundary(&self, addr: u16) -> bool;
    }

    #[derive(Debug)]
    impl Memory for BusDeviceStub {
        fn initialize(&mut self) -> Result<usize, MemoryError>;
        fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
        fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError>;
        fn read_word(&self, addr: u16) -> Result<u16, MemoryError>;
        fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError>;
        #[allow(dead_code)]
        fn dump(&self);
        fn size(&self) -> usize;
        fn as_slice(&mut self) -> &mut [u8];
    }
}

impl Ord for dyn BusDevice {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.get_address_range().0 {
            a if a < other.get_address_range().0 => Ordering::Less,
            a if a > other.get_address_range().0 => Ordering::Greater,
            _ => Ordering::Equal,
        }
    }
}

impl PartialOrd for dyn BusDevice {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Eq for dyn BusDevice {}

impl PartialEq for dyn BusDevice {
    fn eq(&self, other: &Self) -> bool {
        self.get_address_range() == other.get_address_range()
    }
}
