use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::rc::Rc;
use log::debug;
use crate::apu::APUType;
use crate::bus::{Bus, BusError, BusType};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cpu::{CPU, CpuError, CpuType};
use crate::cpu_6502::Cpu6502;
use crate::dummy_device::DummyDevice;
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::ppu::PPUType;

const WRAM_MEMORY_SIZE: usize = 64 * 1024;
const WRAM_START_ADDR: u16 = 0x0000;
const WRAM_END_ADDR: u16 = 0x1FFF;
const PPU_REGISTERS_START_ADDR: u16 = 0x2000;
const PPU_REGISTERS_END_ADDR: u16 = 0x3FFF;
const APU_REGISTERS_START_ADDR: u16 = 0x4000;
const APU_REGISTERS_END_ADDR: u16 = 0x401F;

pub struct NESConsole {
    cpu: Box<dyn CPU>,
    bus: Rc<RefCell<dyn Bus>>,
    devices: Vec<Rc<RefCell<dyn BusDevice>>>
}

impl NESConsole {
    pub fn power_on(&mut self) -> Result<(), NESConsoleError> {
        Ok(())
    }
}

#[derive(Debug)]
pub enum NESConsoleError {
    BuilderError(String),
    IOError(String),
}

impl From<std::io::Error> for NESConsoleError {
    fn from(error: std::io::Error) -> Self {
        NESConsoleError::IOError(error.to_string())
    }
}

impl From<MemoryError> for NESConsoleError {
    fn from(error: MemoryError) -> Self {
        NESConsoleError::IOError(error.to_string())
    }
}

impl From<BusError> for NESConsoleError {
    fn from(error: BusError) -> Self {
        NESConsoleError::BuilderError(error.to_string())
    }
}

impl Display for NESConsoleError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            NESConsoleError::BuilderError(s) => { write!(f, "builder error: {}", s) }
            NESConsoleError::IOError(s) => { write!(f, "i/o error: {}", s) }
        }
    }
}

pub struct NESConsoleBuilder {
    cpu: Option<Box<dyn CPU>>,
    cpu_type: Option<CpuType>,
    cpu_trace_file: Option<File>,
    bus: Option<Rc<RefCell<dyn Bus>>>,
    bus_type: Option<BusType>,
    device_types: Vec<BusDeviceType>,
    devices: Vec<Rc<RefCell<dyn BusDevice>>>,
}

impl NESConsoleBuilder {
    pub fn new() -> Self {
        NESConsoleBuilder {
            cpu: None,
            cpu_type: None,
            cpu_trace_file: None,
            bus: None,
            bus_type: None,
            device_types: Vec::new(),
            devices: Vec::new(),
        }
    }

    pub fn with_cpu_options(mut self, cpu: CpuType, trace_file: Option<File>) -> Self {
        self.cpu_type = Some(cpu);
        self.cpu_trace_file = trace_file;

        self
    }

    pub fn with_bus_type(mut self, bus_type: BusType) -> Self {
        self.bus_type = Some(bus_type);
        self
    }


    pub fn with_bus_device_type(mut self, device_type: BusDeviceType) -> Self {
        self.device_types.push(device_type);
        self
    }

    fn build_cpu(&mut self, bus: Rc<RefCell<dyn Bus>>) -> Result<Box<dyn CPU>, NESConsoleError> {
        debug!("creating CPU: {:?}", self.cpu_type);

        let result: Result<Box<dyn CPU>, NESConsoleError> = match &self.cpu_type {
            Some(CpuType::NES6502) => {
                let cpu = Cpu6502::new(bus, self.cpu_trace_file.take());
                Ok(Box::new(cpu))
            },

            None => {
                Err(NESConsoleError::BuilderError("CPU type not specified".to_string()))
            }
        };

        result
    }

    fn build_bus(&self) -> Result<Rc<RefCell<dyn Bus>>, NESConsoleError> {
        debug!("creating bus: {:?}", self.bus_type);

        let result: Result<Rc<RefCell<dyn Bus>>, NESConsoleError> = match self.bus_type {
            Some(BusType::NESBus) => {
                let bus = NESBus::new();
                Ok(Rc::new(RefCell::new(bus)))
            },

            None => {
                Err(NESConsoleError::BuilderError("bus type not specified".to_string()))
            }
        };

        result
    }

    fn build_wram_device(&self, memory_type: &MemoryType, bus: Rc<RefCell<dyn Bus>>) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {
        let mut wram = match memory_type {
            MemoryType::NESMemory => {
                MemoryBank::new(WRAM_MEMORY_SIZE, bus,(WRAM_START_ADDR, WRAM_END_ADDR))
            },
        };

        wram.initialize()?;

        Ok(Rc::new(RefCell::new(wram)))
    }

    fn build_dummy_device(&self, device_type: &BusDeviceType, bus: Rc<RefCell<dyn Bus>>) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {

        let address_range = match device_type {
            BusDeviceType::WRAM(_) => {
                (WRAM_START_ADDR, WRAM_END_ADDR)
            },
            BusDeviceType::PPU(_) => {
                (PPU_REGISTERS_START_ADDR, PPU_REGISTERS_END_ADDR)
            },
            BusDeviceType::APU(_) => {
                (APU_REGISTERS_START_ADDR, APU_REGISTERS_END_ADDR)
            }
        };

        let dummy = DummyDevice::new(bus, device_type.clone(), address_range);
        Ok(Rc::new(RefCell::new(dummy)))
    }


    fn build_device(&self, device_type: &BusDeviceType, bus: Rc<RefCell<dyn Bus>>) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {
        debug!("creating device: {:?}", device_type);

        let result = match device_type {
            BusDeviceType::WRAM(memory_type) => {
                self.build_wram_device(memory_type, bus)
            },

            BusDeviceType::PPU(_) |
            BusDeviceType::APU(_) => {
                self.build_dummy_device(device_type, bus)
            }
        };

        result
    }

    fn build_nes(mut self) -> Result<NESConsole, NESConsoleError> {

        let bus = self.build_bus()?;

        self.bus = Some(bus.clone());
        self.cpu = Some(self.build_cpu(bus.clone())?);

        for device_type in &self.device_types {
            let device = self.build_device(device_type, bus.clone())?;
            bus.borrow_mut().add_device(device.clone())?;
        }

        let console = NESConsole {
            cpu: self.cpu.take().unwrap(),
            bus: self.bus.take().unwrap(),
            devices: self.devices,
        };

        Ok(console)
    }

    pub fn build(mut self) -> Result<NESConsole, NESConsoleError> {
        if let (Some(_), Some(_)) = (&self.bus_type, &self.cpu_type) {
            self.build_nes()
        } else {
            Err(NESConsoleError::BuilderError("missing required components".to_string()))
        }
    }
}