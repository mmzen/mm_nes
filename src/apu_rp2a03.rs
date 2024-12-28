use log::{debug, info, trace};
use crate::apu::APU;
use crate::apu::ApuType::RP2A03;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};

const APU_NAME: &str = "APU RP2A03";
const APU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x4000, 0x4017);
const APU_EXTERNAL_MEMORY_SIZE: usize = 32;

#[derive(Debug)]
pub struct ApuRp2A03 {}

impl BusDevice for ApuRp2A03 {
    fn get_name(&self) -> String {
        APU_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::APU(RP2A03)
    }

    fn get_address_range(&self) -> (u16, u16) {
        APU_EXTERNAL_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        APU_EXTERNAL_ADDRESS_SPACE.0 <= addr && addr <= APU_EXTERNAL_ADDRESS_SPACE.1
    }
}

impl Memory for ApuRp2A03 {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing APU");
        Ok(APU_EXTERNAL_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        trace!("APU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);

        let _ = match addr {
            _ => debug!("APU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000),
        };

        Ok(0)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        trace!("APU: registers access: write byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);

        let _ = match addr {
            _ => debug!("APU: registers access: write byte at 0x{:04X} (0x{:04X}): 0x{:02X}", addr, addr + 0x4000, value),
        };

        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(0)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        APU_EXTERNAL_MEMORY_SIZE
    }
}

impl ApuRp2A03 {
    pub fn new() -> Self {
        ApuRp2A03 {}
    }
}

impl APU for ApuRp2A03 {}