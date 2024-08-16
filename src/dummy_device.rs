use std::cell::RefCell;
use std::rc::Rc;
use log::info;
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};

const DEFAULT_CONTENT: u8 = 0x00;
const DEVICE_NAME: &str = "Dummy Device";

#[derive(Debug)]
pub struct DummyDevice {
    bus: Rc<RefCell<dyn Bus>>,
    device_type: BusDeviceType,
    address_range: (u16, u16),
    memory: Vec<u8>,
}

impl DummyDevice {
    pub fn new(bus: Rc<RefCell<dyn Bus>>, device_type: BusDeviceType, address_range:(u16, u16)) -> Self {
        DummyDevice {
            bus,
            device_type,
            address_range,
            memory: vec![DEFAULT_CONTENT],
        }
    }
}

impl BusDevice for DummyDevice {
    fn get_name(&self) -> String {
        self.device_type.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_address_range(&self) -> (u16, u16) {
        self.address_range
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        self.address_range.0 <= addr && addr <= self.address_range.1
    }
}

impl Memory for DummyDevice {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(1)
    }

    fn read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(DEFAULT_CONTENT)
    }

    fn write_byte(&mut self, _: u16, _: u8) -> Result<(), MemoryError> {
        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(DEFAULT_CONTENT as u16)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
        info!("dummy device: {}", DEFAULT_CONTENT);
    }

    fn size(&self) -> usize {
        1
    }

    fn as_slice(&mut self) -> &mut [u8] {
        self.memory.as_mut_slice()
    }
}