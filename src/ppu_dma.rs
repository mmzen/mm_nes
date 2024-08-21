use std::cell::RefCell;
use std::rc::Rc;
use log::{debug, info};
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::dma::{Dma, DmaError, DmaType};
use crate::dma::PpuDmaType::NESPPUDMA;
use crate::dma_device::DmaDevice;
use crate::memory::{Memory, MemoryError};

const PPU_DMA_NAME: &str = "PPU DMA";
const PPU_DMA_ADDRESS_SPACE: (u16, u16) = (0x4014, 0x4014);
const PPU_DMA_SIZE: usize = 1;

#[derive(Debug)]
pub struct PpuDma {
    device: Rc<RefCell<dyn DmaDevice>>,
    last_transfer_addr: u8,
    bus: Rc<RefCell<dyn Bus>>
}

impl Dma for PpuDma {
    fn transfer_memory(&mut self, value: u8) -> Result<u16, MemoryError> {
        let source = (value as u16) << 8;
        let last_value = source | 0x00FF;

        debug!("transferring 256 bytes of memory from 0x{:04X} to PPU", source);

        let mut index = 0;

        for addr in source..=last_value {
            let data = self.bus.borrow().read_byte(addr)?;
            self.device.borrow_mut().dma_write(index as u8, data)?;
            index += 1;
        }

        Ok(index)
    }
}

impl BusDevice for PpuDma {
    fn get_name(&self) -> String {
        PPU_DMA_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::DMA(DmaType::PpuDma(NESPPUDMA))
    }

    fn get_address_range(&self) -> (u16, u16) {
        PPU_DMA_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        PPU_DMA_ADDRESS_SPACE.0 <= addr && addr <= PPU_DMA_ADDRESS_SPACE.1
    }
}

impl Memory for PpuDma {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing PPU DMA");
        Ok(PPU_DMA_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let effective_addr = self.get_effective_address(addr);

        let value = match effective_addr {
            0x4014 => self.last_transfer_addr,
            _ => unreachable!()
        };

        Ok(value)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.transfer_memory(value)?;
        self.last_transfer_addr = value;

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
        PPU_DMA_SIZE
    }
}

impl PpuDma {

    pub fn new(device: Rc<RefCell<dyn DmaDevice>>, bus: Rc<RefCell<dyn Bus>>) -> Self {
        PpuDma {
            device,
            last_transfer_addr: 0,
            bus
        }
    }

    fn get_effective_address(&self, addr: u16) -> u16 {
        PPU_DMA_ADDRESS_SPACE.0 + (addr & (PPU_DMA_SIZE as u16 - 1))
    }
}

