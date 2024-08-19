use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use std::rc::Rc;
use log::debug;
use crate::bus::{Bus, BusError, BusType};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::cpu::{CPU, CpuError, CpuType};
use crate::cpu_6502::Cpu6502;
use crate::ines_loader::INesLoader;
use crate::loader::{Loader, LoaderError, LoaderType};
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::ppu::{PpuError, PpuNameTableMirroring, PpuType};
use crate::ppu_2c02::Ppu2c02;

const WRAM_MEMORY_SIZE: usize = 2 * 1024;
const WRAM_START_ADDR: u16 = 0x0000;
const WRAM_END_ADDR: u16 = 0x1FFF;
const DEFAULT_START_ADDRESS: u16 = 0xFFFC;

pub struct NESConsole {
    cpu: Box<dyn CPU>,
    entry_point: Option<u16>,
}

impl NESConsole {
    pub fn power_on(&mut self) -> Result<(), NESConsoleError> {
        self.cpu.initialize()?;

        let result = if let Some(pc) = self.entry_point {
            self.cpu.run_with_pc_immediate(pc)
        } else {
            self.cpu.run_with_pc_indirect(DEFAULT_START_ADDRESS)
        };

        if let Err(error) = result {
            self.cpu.panic(&error);
            Err(NESConsoleError::CpuError("giving up".to_string()))
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub enum NESConsoleError {
    BuilderError(String),
    IOError(String),
    ProgramLoaderError(String),
    CpuError(String),
    PpuError(String),
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

impl From<CpuError> for NESConsoleError {
    fn from(error: CpuError) -> Self {
        NESConsoleError::CpuError(error.to_string())
    }
}

impl From<BusError> for NESConsoleError {
    fn from(error: BusError) -> Self {
        NESConsoleError::BuilderError(error.to_string())
    }
}

impl From<LoaderError> for NESConsoleError {
    fn from(error: LoaderError) -> Self {
        NESConsoleError::ProgramLoaderError(error.to_string())
    }
}

impl From<PpuError> for NESConsoleError {
    fn from(error: PpuError) -> Self {
        NESConsoleError::PpuError(error.to_string())
    }
}

impl Display for NESConsoleError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            NESConsoleError::BuilderError(s) => { write!(f, "builder error: {}", s) }
            NESConsoleError::IOError(s) => { write!(f, "i/o error: {}", s) }
            NESConsoleError::ProgramLoaderError(s) => { write!(f, "program loader error: {}", s) }
            NESConsoleError::CpuError(s) => { write!(f, "cpu error: {}", s) }
            NESConsoleError::PpuError(s) => { write!(f, "ppu error: {}", s) }
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
    loader_type: Option<LoaderType>,
    rom_file: Option<String>,
    entry_point: Option<u16>,
    cartridge: Option<Rc<RefCell<dyn Cartridge>>>,
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
            loader_type: None,
            rom_file: None,
            entry_point: None,
            cartridge: None,
        }
    }

    pub fn with_loader_type(mut self, loader_type: LoaderType) -> Self {
        self.loader_type = Some(loader_type);
        self
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

    pub fn with_rom_file(mut self, rom_file: String) -> Self {
        debug!("setting rom file: {}", rom_file);

        self.rom_file = Some(rom_file);
        self
    }

    pub fn with_entry_point(mut self, entry_point: Option<u16>) -> Self {
        self.entry_point = entry_point;
        self
    }

    fn build_cpu(&mut self, bus: Rc<RefCell<dyn Bus>>) -> Result<Box<dyn CPU>, NESConsoleError> {
        debug!("creating cpu: {:?}", self.cpu_type.clone().unwrap());

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
        debug!("creating bus: {:?}", self.bus_type.clone().unwrap());

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

    fn build_wram_device(&self, memory_type: &MemoryType) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {
        debug!("creating wram: {:?}", memory_type);

        let mut wram = match memory_type {
            MemoryType::NESMemory => {
                MemoryBank::new(WRAM_MEMORY_SIZE, (WRAM_START_ADDR, WRAM_END_ADDR))
            },
        };

        wram.initialize()?;
        Ok(Rc::new(RefCell::new(wram)))
    }

    fn build_ppu_device(&self, ppu_type: &PpuType, chr_rom: Rc<RefCell<dyn BusDevice>>, mirroring: PpuNameTableMirroring) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {
        debug!("creating ppu {:?}", ppu_type);

        let mut ppu = match ppu_type {
            PpuType::NES2C02 => {
                Ppu2c02::new(chr_rom, mirroring)?
            },
        };

        ppu.initialize()?;
        Ok(Rc::new(RefCell::new(ppu)))
    }

    fn build_cartridge_device(&self) -> Result<Rc<RefCell<dyn Cartridge>>, NESConsoleError> {
        debug!("creating cartridge");

        if let Some(ref rom_file) = self.rom_file {
            let rom_path = PathBuf::from(rom_file);
            let mut loader = self.build_loader()?;
            let cartridge = loader.load(&rom_path)?;

            Ok(cartridge)
        } else {
            Err(NESConsoleError::BuilderError("rom file not specified".to_string()))
        }
    }

    fn build_device_and_connect_to_bus(&mut self, device_type: &BusDeviceType, bus: Rc<RefCell<dyn Bus>>) -> Result<(), NESConsoleError> {
        debug!("creating device: {:?}", device_type);

        match device_type {
            BusDeviceType::CARTRIDGE(_) => {
                let cartridge = self.build_cartridge_device()?;
                bus.borrow_mut().add_device(cartridge.borrow().get_prg_rom())?;
                self.cartridge = Some(cartridge);
            },

            BusDeviceType::WRAM(memory_type) => {
                let memory = self.build_wram_device(memory_type)?;
                bus.borrow_mut().add_device(memory)?;
            },

            BusDeviceType::PPU(ppu_type) => {
                let chr_rom = self
                    .cartridge
                    .as_ref()
                    .map(|cartridge| cartridge.borrow().get_chr_rom())
                    .ok_or(NESConsoleError::BuilderError("no cartridge to load".to_string()))?;

                let mirroring = self
                    .cartridge
                    .as_ref()
                    .map(|cartridge| cartridge.borrow().get_mirroring())
                    .ok_or(NESConsoleError::BuilderError("ppu mirroring not set".to_string()))?;

                let ppu = self.build_ppu_device(ppu_type, chr_rom, mirroring)?;
                bus.borrow_mut().add_device(ppu)?;
            },

            _ => {}
        };

        Ok(())
    }

    fn build_loader(&self) -> Result<Box<dyn Loader>, NESConsoleError> {
        debug!("creating loader: {:?}", self.loader_type.clone().unwrap());

        match self.loader_type {
            None => {
                Err(NESConsoleError::BuilderError("loader not set".to_string()))
            },
            Some(LoaderType::INESV1) => {
                Ok(INesLoader::new())
            }
        }
    }

    fn build_nes(mut self) -> Result<NESConsole, NESConsoleError> {

        let bus = self.build_bus()?;

        self.bus = Some(bus.clone());
        self.cpu = Some(self.build_cpu(bus.clone())?);

        let device_types = self.device_types.clone();

        for device_type in device_types {
            self.build_device_and_connect_to_bus(&device_type, bus.clone())?;
        }

        let cpu = self.cpu.take()
            .ok_or(NESConsoleError::BuilderError("cpu missing".to_string()))?;

        let console = NESConsole {
            cpu,
            entry_point: self.entry_point.take()
        };

        Ok(console)
    }

    pub fn build(self) -> Result<NESConsole, NESConsoleError> {
        if let (Some(_), Some(_), Some(_), Some(_)) = (&self.bus_type, &self.cpu_type, &self.loader_type, &self.rom_file) {
            self.build_nes()
        } else {
            Err(NESConsoleError::BuilderError("missing required components".to_string()))
        }
    }
}