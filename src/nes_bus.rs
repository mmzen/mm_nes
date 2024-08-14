use std::cell::RefCell;
use std::fmt::Debug;
use std::rc::Rc;
use log::debug;
use crate::memory::{Memory, MemoryError};

const WRAM_START_ADDR: u16 = 0x0000;
const WRAM_END_ADDR: u16 = 0x1FFF;
const PPU_REGISTERS_START_ADDR: u16 = 0x2000;
const PPU_REGISTERS_END_ADDR: u16 = 0x3FFF;
const APU_REGISTERS_START_ADDR: u16 = 0x4000;
const APU_REGISTERS_END_ADDR: u16 = 0x401F;
const CARTRIDGE_START_ADDR: u16 = 0x4020;
const CARTRIDGE_END_ADDR: u16 = 0xFFFF;
const ADDRESSABLE_MEMORY_SIZE: usize = 64 * 1024;


#[derive(Debug)]
pub struct NESBus {
    last_effective_addr: Option<(Rc<RefCell<dyn Memory>>, u16)>,
    wram: Rc<RefCell<dyn Memory>>,
    ppu: Rc<RefCell<dyn Memory>>,
    apu: Rc<RefCell<dyn Memory>>,
    cartridge: Rc<RefCell<dyn Memory>>,
}

impl Memory for NESBus {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(ADDRESSABLE_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let (memory, effective_addr) = self.lookup_address(addr)?;
        let value = memory.borrow().read_byte(effective_addr)?;

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

    fn is_addr_in_boundary(&self, addr: u16) -> bool {
        match addr {
            WRAM_START_ADDR..=WRAM_END_ADDR |
            PPU_REGISTERS_START_ADDR..=PPU_REGISTERS_END_ADDR |
            APU_REGISTERS_START_ADDR..=APU_REGISTERS_END_ADDR |
            CARTRIDGE_START_ADDR..=CARTRIDGE_END_ADDR => true,
            _ => false,
        }
    }

    fn size(&self) -> usize {
        ADDRESSABLE_MEMORY_SIZE
    }

    fn as_slice(&mut self) -> &mut [u8] {
        todo!()
    }
}

impl NESBus {

    pub fn new(wram: Rc<RefCell<dyn Memory<>>>, ppu: Rc<RefCell<dyn Memory<>>>, apu: Rc<RefCell<dyn Memory<>>>, cartridge: Rc<RefCell<dyn Memory<>>>) -> Self {
        NESBus {
            last_effective_addr: None,
            wram,
            ppu,
            apu,
            cartridge,
        }
    }

    fn lookup_address(&self, addr: u16) -> Result<(Rc<RefCell<dyn Memory>>, u16), MemoryError> {
        match addr {
            WRAM_START_ADDR..=WRAM_END_ADDR => {
                let effective_addr = addr & 0x07FF;
                debug!("translated address 0x{:04X} to wram at 0x{:04X}", addr, effective_addr);

                Ok((Rc::clone(&self.wram), effective_addr))
            },

            PPU_REGISTERS_START_ADDR..=PPU_REGISTERS_END_ADDR => {
                let effective_addr = addr & 0x2007;
                debug!("translated address 0x{:04X} to ppu_registers at 0x{:04X}", addr, effective_addr);

                Ok((Rc::clone(&self.ppu), effective_addr))
            },

            APU_REGISTERS_START_ADDR..=APU_REGISTERS_END_ADDR => {
                let effective_addr = addr & 0x4017;
                debug!("translated address 0x{:04X} to apu_registers at 0x{:04X}", addr, effective_addr);

                Ok((Rc::clone(&self.apu), effective_addr))
            },

            CARTRIDGE_START_ADDR..=CARTRIDGE_END_ADDR => {
                let effective_addr = addr & 0xFFFF;
                debug!("translated address 0x{:04X} to cartridge at 0x{:04X}", addr, effective_addr);

                Ok((Rc::clone(&self.cartridge), effective_addr))
            },

            _ => {
                debug!("open bus error: address 0x{:04X} is out of range", addr);
                if let Some((ref memory, effective_address)) = self.last_effective_addr {
                    debug!("open bus error: returning last effective address: 0x{:04X}", effective_address);
                    Ok((Rc::clone(memory), effective_address))
                } else {
                    Err(MemoryError::OutOfRange(addr))
                }
            }
        }
    }

    #[cfg(test)]
    pub fn wram(&self) -> Rc<RefCell<dyn Memory>> {
        self.wram.clone()
    }

    #[cfg(test)]
    pub fn ppu(&self) -> Rc<RefCell<dyn Memory>> {
        self.ppu.clone()
    }

    #[cfg(test)]
    pub fn apu(&self) -> Rc<RefCell<dyn Memory>> {
        self.apu.clone()
    }

    #[cfg(test)]
    pub fn cartridge(&self) -> Rc<RefCell<dyn Memory>> {
        self.cartridge.clone()
    }

    #[cfg(test)]
    pub fn last_effective_addr(&self) -> Option<(Rc<RefCell<dyn Memory>>, u16)> {
        self.last_effective_addr.clone()
    }
}

