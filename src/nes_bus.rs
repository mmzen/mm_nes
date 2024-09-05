use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use log::{debug, trace};
use crate::bus::{Bus, BusError};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};

pub const BUS_ADDRESSABLE_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub struct NESBus {
    devices: Vec<Rc<RefCell<dyn BusDevice>>>,
    num_devices: usize,
}

impl Memory for NESBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(BUS_ADDRESSABLE_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().read_byte(effective_addr)?;

        Ok(value)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().trace_read_byte(effective_addr)?;

        Ok(value)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        memory.borrow_mut().write_byte(effective_addr, value)?;

        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().read_word(effective_addr)?;

        Ok(value)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        memory.borrow_mut().write_word(effective_addr, value)?;

        Ok(())
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        BUS_ADDRESSABLE_SIZE
    }
}

impl Bus for NESBus {


    fn add_device(&mut self, device: Rc<RefCell<dyn BusDevice>>) -> Result<(), BusError> {
        let size = device.borrow().size();
        let address_space = device.borrow().get_address_range();

        debug!("BUS: adding device {} - size: {} bytes, address range: 0x{:04X} - 0x{:04X}",
        device.borrow().get_name(), size, address_space.0, address_space.1);

        for addr in address_space.0..=address_space.1 {
           self.devices[addr as usize] = device.clone();
        }

        let count = self.count_addresses_in_bus();
        self.num_devices += 1;

        debug!("BUS: {} addresses mapped in bus, {} devices", count, self.num_devices);

        Ok(())
    }
}

impl NESBus {

    pub fn new() -> Self {
        let open_bus = Rc::new(RefCell::new(OpenBus::new()));

        NESBus {
            devices: vec![open_bus.clone(); 65536],
            num_devices: 0,
        }
    }

    #[allow(dead_code)]
    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        addr < BUS_ADDRESSABLE_SIZE as u16
    }

    fn lookup_address(&self, addr: u16) -> Result<(Rc<RefCell<dyn BusDevice>>, u16), BusError> {
        let device = self.devices[addr as usize].clone();
        // TODO it crashes here with scroll.nes rom, reading at 0x0000, and get a NESMemory device with size 0
        //println!("BUS: looking up address 0x{:04X}, size: {} - {}", addr, device.borrow().size(), device.borrow().get_device_type());
        let effective_addr = addr & (device.borrow().size() - 1) as u16;

        trace!("BUS: translated address 0x{:04X} to device {} ({}, 0x{:04X} - 0x{:04X}), effective address 0x{:04X}",
                    addr, device.borrow().get_name(), device.borrow().get_device_type(),
                    device.borrow().get_address_range().0, device.borrow().get_address_range().1,
                    effective_addr);

        Ok((device, effective_addr))
    }

    fn count_addresses_in_bus(&self) -> usize {
        self.devices
            .iter()
            .filter(|d| {
                d.borrow().get_device_type() != BusDeviceType::OPENBUS
            })
            .count()
    }
}

const OPEN_BUS_DEVICE_NAME: &str = "Open Bus";

#[derive(Debug)]
struct OpenBus {
    last_value: u8,
}

impl OpenBus {
    fn new() -> Self {
        OpenBus {
            last_value: 0x00,
        }
    }
}

impl BusDevice for OpenBus {
    fn get_name(&self) -> String {
        OPEN_BUS_DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::OPENBUS
    }

    #[allow(arithmetic_overflow)]
    fn get_address_range(&self) -> (u16, u16) {
        (0x000, 0x0000 + (BUS_ADDRESSABLE_SIZE - 1) as u16)
    }

    fn is_addr_in_address_space(&self, _: u16) -> bool {
        panic!("open bus does not have an address range");
    }
}

impl Memory for OpenBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(0)
    }

    fn read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(self.last_value)
    }

    fn trace_read_byte(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(0x00)
    }

    fn write_byte(&mut self, _: u16, _: u8) -> Result<(), MemoryError> {
        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(0x0000)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
    }

    fn size(&self) -> usize {
        1
    }
}
