use std::cell::RefCell;
use std::rc::Rc;
use log::debug;
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::memory::MemoryType::NESMemory;

pub const MEMORY_BASE_ADDRESS: usize = 0x0000;
const DEVICE_NAME: &str = "Memory Bank";

#[derive(Debug)]
pub struct MemoryBank {
    memory: Vec<u8>,
    bus: Rc<RefCell<dyn Bus>>,
    address_space: (u16, u16),
    address_space_size: usize,
    device_type: BusDeviceType,
}

impl Memory for MemoryBank {
    #[allow(dead_code)]
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        debug!("initializing memory: {} kB to 0x{:04X}, 0x{:04X}", self.memory.len() / 1024, MEMORY_BASE_ADDRESS, MEMORY_BASE_ADDRESS + self.memory.len() - 1);

        self.memory.fill(0x00);
        Ok(self.size())
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        //debug!("reading byte at 0x{:04X}", addr);

        if !self.addr_is_in_boundary(addr) {
            Err(MemoryError::OutOfRange(addr))
        } else {
            let value = self.memory[addr as usize];
            //debug!("read byte at 0x{:04X}: {:02X}", addr, value);
            Ok(value)
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        //debug!("writing byte ({:02X}) at 0x{:04X}", value, addr);

        if !self.addr_is_in_boundary(addr) {
            Err(MemoryError::OutOfRange(addr))
        } else {
            self.memory[addr as usize] = value;
            Ok(())
        }
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.read_byte(addr)?;
        let next = self.wrapping_add(addr, 1);
        let hi = self.read_byte(next)?;

        Ok((hi as u16) << 8 | lo as u16)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let lower_byte = value as u8;
        let upper_byte = ((value & 0xFF00) >> 8) as u8;
        let next = self.wrapping_add(addr, 1);

        self.write_byte(addr, lower_byte)?;
        self.write_byte(next, upper_byte)?;

        Ok(())
    }

    fn dump(&self) {
        for (index, byte) in self.memory.as_slice().iter().enumerate() {
            if index % 16 == 0 {
                if index > 0 {
                    println!();
                }
                print!("0x{:04X}: ", index);
            }
            print!("{:02X} ", byte);
        }
        println!();
    }

    fn size(&self) -> usize {
        self.memory.len()
    }

    fn as_slice(&mut self) -> &mut [u8] {
        self.memory.as_mut_slice()
    }
}

impl BusDevice for MemoryBank {
    fn get_name(&self) -> String {
        DEVICE_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        self.device_type.clone()
    }

    fn get_address_range(&self) -> (u16, u16) {
        self.address_space
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        self.address_space.0 <= addr && addr <= self.address_space.1
    }
}

impl MemoryBank {
    pub(crate) fn new(size: usize, bus: Rc<RefCell<dyn Bus>>, address_range: (u16, u16)) -> Self {
        let address_space_size = (address_range.1 - address_range.0 + 1) as usize;

        MemoryBank {
            memory: vec![0x00; size],
            bus,
            address_space: address_range,
            address_space_size,
            device_type: BusDeviceType::WRAM(NESMemory),
        }
    }

    fn wrapping_add(&self, addr: u16, n: u16) -> u16 {
        if addr == self.address_space.1 {
            self.address_space.0
        } else {
            addr + n
        }
    }

    fn addr_is_in_boundary(&self, addr: u16) -> bool {
        (addr as usize) < self.memory.len()
    }
}