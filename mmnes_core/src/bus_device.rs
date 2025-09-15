use std::cmp::Ordering;
use std::fmt;
use std::fmt::{Display, Formatter};
#[cfg(test)]
use mockall::mock;
use crate::apu::ApuType;
use crate::cartridge::CartridgeType;
use crate::controller::ControllerType;
use crate::dma::DmaType;
use crate::memory::{Memory, MemoryType};
#[cfg(test)]
use crate::memory::{MemoryError};
use crate::ppu::PpuType;

#[derive(Debug, Clone)]
pub enum BusDeviceType {
    WRAM(MemoryType),
    PPU(PpuType),
    APU(ApuType),
    CARTRIDGE(CartridgeType),
    DMA(DmaType),
    CONTROLLER(ControllerType),
    OPENBUS
}

impl Display for BusDeviceType {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            BusDeviceType::WRAM(memory) => write!(f, "device type: WRAM - {}", memory),
            BusDeviceType::PPU(ppu) => write!(f, "device type: PPU - {}", ppu),
            BusDeviceType::APU(apu) => write!(f, "device type: APU - {}", apu),
            BusDeviceType::CARTRIDGE(cartridge) => write!(f, "device type: CARTRIDGE - {}", cartridge),
            BusDeviceType::DMA(dma) => { write!(f, "device type: DMA - {}", dma) }
            BusDeviceType::OPENBUS => { write!(f, "device type: OPEN BUS") },
            BusDeviceType::CONTROLLER(controller) => { write!(f, "device type: CONTROLLER - {}", controller) }
        }
    }
}

impl PartialEq for BusDeviceType {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (BusDeviceType::WRAM(a), BusDeviceType::WRAM(b)) => a == b,
            (BusDeviceType::PPU(a), BusDeviceType::PPU(b)) => a == b,
            (BusDeviceType::APU(a), BusDeviceType::APU(b)) => a == b,
            (BusDeviceType::CARTRIDGE(a), BusDeviceType::CARTRIDGE(b)) => a == b,
            (BusDeviceType::CONTROLLER(a), BusDeviceType::CONTROLLER(b)) => a == b,
            (BusDeviceType::OPENBUS, BusDeviceType::OPENBUS) => true,
            _ => false,
        }
    }
}

pub trait BusDevice: Memory {
    fn get_name(&self) -> String;
    fn get_device_type(&self) -> BusDeviceType;
    fn get_virtual_address_range(&self) -> (u16, u16);
}

impl Ord for dyn BusDevice {
    fn cmp(&self, other: &Self) -> Ordering {
        match self.get_virtual_address_range().0 {
            a if a < other.get_virtual_address_range().0 => Ordering::Less,
            a if a > other.get_virtual_address_range().0 => Ordering::Greater,
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
        self.get_virtual_address_range() == other.get_virtual_address_range()
    }
}

#[cfg(test)]
mock! {
    #[derive(Debug)]
    pub BusDeviceStub {}

    impl BusDevice for BusDeviceStub {
        fn get_name(&self) -> String;
        fn get_device_type(&self) -> BusDeviceType;
        fn get_virtual_address_range(&self) -> (u16, u16);
    }

    #[derive(Debug)]
    impl Memory for BusDeviceStub {
        fn initialize(&mut self) -> Result<usize, MemoryError>;
        fn read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
        fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError>;
        fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError>;
        fn read_word(&self, addr: u16) -> Result<u16, MemoryError>;
        fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError>;
        #[allow(dead_code)]
        fn dump(&self);
        fn size(&self) -> usize;
    }
}
