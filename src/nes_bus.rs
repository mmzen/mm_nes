use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use log::{debug, trace};
use crate::bus::{Bus, BusError};
use crate::bus_device::BusDevice;
use crate::memory::{Memory, MemoryError};

pub const BUS_ADDRESSABLE_SIZE: usize = 64 * 1024;

#[derive(Debug)]
pub struct NESBus {
    last_effective_addr: Option<(Rc<RefCell<dyn BusDevice>>, u16)>,
    devices: Vec<Rc<RefCell<dyn BusDevice>>>,
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

        self.devices.push(device);
        self.devices.sort();
        debug!("BUS: {} devices attached to the bus", self.devices.len());

        Ok(())
    }
}

impl NESBus {

    pub fn new() -> Self {
        NESBus {
            last_effective_addr: None,
            devices: vec![],
        }
    }

    #[allow(dead_code)]
    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        addr < BUS_ADDRESSABLE_SIZE as u16
    }

    fn lookup_address(&self, addr: u16) -> Result<(Rc<RefCell<dyn BusDevice>>, u16), BusError> {
        for device in &self.devices {
            if device.borrow().is_addr_in_address_space(addr) {
                let d = device.borrow();
                let effective_addr = addr & (d.size() - 1) as u16;

                trace!("BUS: translated address 0x{:04X} to device {} ({}, 0x{:04X} - 0x{:04X}), effective address 0x{:04X}",
                    addr, d.get_name(), d.get_device_type(), d.get_address_range().0, d.get_address_range().1, effective_addr);

                return Ok((Rc::clone(&device), effective_addr));
            }
        }

        debug!("BUS: open bus error: address 0x{:04X} is out of range", addr);

        if let Some((ref device, effective_address)) = self.last_effective_addr {
            debug!("BUS: open bus error: returning last effective address: 0x{:04X}", effective_address);
            Ok((Rc::clone(device), effective_address))
        } else {
            Err(BusError::Unmapped(addr))
        }
    }

    #[cfg(test)]
    pub fn last_effective_addr(&self) -> Option<(Rc<RefCell<dyn BusDevice>>, u16)> {
        self.last_effective_addr.clone()
    }
}

