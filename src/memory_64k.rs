use log::debug;

use crate::memory::{Memory, MemoryError};

const MEMORY_SIZE: usize = 64 * 1024;
const MEMORY_BASE_ADDRESS: usize = 0x0000;
const MEMORY_END_ADDRESS: usize = MEMORY_BASE_ADDRESS + MEMORY_SIZE - 1;

#[derive(Debug)]
pub struct Memory64k {
    memory: [u8; MEMORY_SIZE]
}

impl Memory for Memory64k {
    #[allow(dead_code)]
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        debug!("initializing memory: {} kB to 0x{:04X}, 0x{:04X}", self.memory.len() / 1024, MEMORY_BASE_ADDRESS, MEMORY_END_ADDRESS);

        self.memory.fill(0);
        Ok(MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        debug!("reading byte at 0x{:04X}", addr);

        if !self.is_addr_in_boundary(addr) {
            Err(MemoryError::OutOfBounds(addr))
        } else {
            Ok(self.memory[addr as usize])
        }
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<u8, MemoryError> {
        debug!("writing byte at 0x{:04X}", addr);

        if !self.is_addr_in_boundary(addr) {
            Err(MemoryError::OutOfBounds(addr))
        } else {
            self.memory[addr as usize] = value;
            Ok(value)
        }
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        let lower_byte = self.read_byte(addr)?;
        let next = addr.checked_add(1).ok_or(MemoryError::OutOfBounds(addr))?;

        let upper_byte = self.read_byte(next)?;

        Ok((upper_byte as u16) << 8 | lower_byte as u16)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<u16, MemoryError> {
        let lower_byte = (value & 0x00FF) as u8;
        let upper_byte = ((value & 0xFF00) >> 8) as u8;
        let next = addr.checked_add(1).ok_or(MemoryError::OutOfBounds(addr))?;

        self.write_byte(addr, lower_byte)?;
        self.write_byte(next, upper_byte)?;
        Ok(value)
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
        MEMORY_SIZE
    }

    fn as_slice(&mut self) -> &mut [u8] {
        self.memory.as_mut_slice()
    }
}

impl Default for Memory64k {
    fn default() -> Self {
        Memory64k {
            memory: [0xFF; MEMORY_SIZE],
        }
    }
}

impl Memory64k {
}