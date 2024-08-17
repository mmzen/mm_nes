use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use log::info;
use crate::bus::{Bus, BusError};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::ppu::{PPU, PpuError, PpuType};

const PPU_WRAM_SIZE: usize = 2 * 1024;
const PPU_WRAM_ADDR_START: u16 = 0x0000;
const PPU_WRAM_ADDR_END: u16 = 0x3FFF;
const PPU_NAME: &str = "PPU 2C02";
const PPU_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x3FFF);
const PPU_MEMORY_SIZE: usize = 8;

pub struct Ppu2c02 {
    register: Register,
    internal_bus: Rc<RefCell<dyn Bus>>,
    bus: Rc<RefCell<dyn Bus>>,
    address_space: (u16, u16),
    address_space_size: usize,
    device_type: BusDeviceType,
}

#[derive(Debug)]
struct Register {
    controller: u8,
    mask: u8,
    status: u8,
    oam_addr: u8,
    oam_data: u8,
    scroll: u8,
    addr: u16,
    data: u8,
    oam_dma: u8
}

impl Register {
    fn new() -> Self {
        Register {
            controller: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam_data: 0,
            scroll: 0,
            addr: 0,
            data: 0,
            oam_dma: 0
        }
    }
}

impl PPU for Ppu2c02 {
    fn reset(&mut self) -> Result<(), PpuError> {
        self.register.controller = 0;
        self.register.mask = 0;
        self.register.scroll = 0;
        self.register.data = 0;
        Ok(())
    }

    fn initialize(&mut self) -> Result<(), PpuError> {
        info!("initializing PPU");
        Ok(())
    }

    fn panic(&self, _: &PpuError) {
        todo!()
    }
}

impl Memory for Ppu2c02 {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        Ok(PPU_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.internal_bus.borrow_mut().read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        self.internal_bus.borrow_mut().write_byte(addr, value)
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        self.internal_bus.borrow_mut().read_word(addr)
    }

    fn write_word(&mut self, addr: u16, value: u16) -> Result<(), MemoryError> {
        self.internal_bus.borrow_mut().write_word(addr, value)
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        PPU_MEMORY_SIZE
    }

    fn as_slice(&mut self) -> &mut [u8] {
        todo!()
    }
}

impl Debug for Ppu2c02 {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        todo!()
    }
}

impl BusDevice for Ppu2c02 {
    fn get_name(&self) -> String {
        PPU_NAME.to_string()
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

impl Ppu2c02 {
    pub fn new(bus: Rc<RefCell<dyn Bus>>) -> Result<Self, PpuError> {
        let internal_bus = Rc::new(RefCell::new(NESBus::new()));
        let memory = Rc::new(RefCell::new(MemoryBank::new(PPU_WRAM_SIZE, bus.clone(), (PPU_WRAM_ADDR_START, PPU_WRAM_ADDR_END))));

        memory.borrow_mut().initialize()?;
        internal_bus.borrow_mut().add_device(memory)?;

        let ppu = Ppu2c02 {
            register: Register::new(),
            internal_bus,
            bus,
            address_space: PPU_ADDRESS_SPACE,
            address_space_size: PPU_MEMORY_SIZE,
            device_type: BusDeviceType::PPU(PpuType::NES2C02)
        };

        Ok(ppu)
    }
}