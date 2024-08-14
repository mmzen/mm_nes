use log::debug;

use crate::memory::{Memory, MemoryError};

pub const MEMORY_DEFAULT_SIZE: usize = 64 * 1024;
pub const MEMORY_BASE_ADDRESS: usize = 0x0000;


#[derive(Debug)]
pub struct MemoryBank {
    memory: Vec<u8>
}

impl Memory for MemoryBank {
    #[allow(dead_code)]
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        debug!("initializing memory: {} kB to 0x{:04X}, 0x{:04X}", self.memory.len() / 1024, MEMORY_BASE_ADDRESS, MEMORY_BASE_ADDRESS + self.memory.len() - 1);

        self.memory.fill(0xFF);
        Ok(self.size())
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        //debug!("reading byte at 0x{:04X}", addr);

        if !self.is_addr_in_boundary(addr) {
            Err(MemoryError::OutOfRange(addr))
        } else {
            let value = self.memory[addr as usize];
            debug!("read byte at 0x{:04X}: {:02X}", addr, value);
            Ok(value)
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        debug!("writing byte ({:02X}) at 0x{:04X}", value, addr);

        if !self.is_addr_in_boundary(addr) {
            Err(MemoryError::OutOfRange(addr))
        } else {
            self.memory[addr as usize] = value;
            Ok(())
        }
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lo = self.read_byte(addr)?;
        let next = addr.wrapping_add(1) % self.size() as u16;
        let hi = self.read_byte(next)?;

        Ok((hi as u16) << 8 | lo as u16)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        let lower_byte = value as u8;
        let upper_byte = ((value & 0xFF00) >> 8) as u8;
        let next = addr.wrapping_add(1) % self.size() as u16;

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

    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        (addr as usize) < self.memory.len()
    }

    fn size(&self) -> usize {
        self.memory.len()
    }

    fn as_slice(&mut self) -> &mut [u8] {
        self.memory.as_mut_slice()
    }
}

impl Default for MemoryBank {
    fn default() -> Self {
        MemoryBank {
            memory: vec![0xFF; MEMORY_DEFAULT_SIZE],
        }
    }
}

impl MemoryBank {
    pub(crate) fn new_with_size(size: u16) -> Self {
        MemoryBank {
            memory: vec![0xFF; size as usize],
        }
    }
}