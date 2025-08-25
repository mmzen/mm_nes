use std::cell::RefCell;
use std::fmt::{Display, Formatter};
use std::fs::File;
use std::path::PathBuf;
use std::rc::Rc;
use log::debug;
use crate::apu::{ApuError, ApuType, APU};
use crate::apu_rp2a03::ApuRp2A03;
use crate::bus::{Bus, BusError, BusType};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::controller::{Controller, ControllerType};
use crate::cpu::{CPU, CpuError, CpuType};
use crate::cpu_6502::Cpu6502;
use crate::dma::PpuDmaType;
use crate::dma_device::DmaDevice;
use crate::ines_loader::INesLoader;
use crate::input::InputError;
use crate::input_external::InputExternal;
use crate::key_event::KeyEvents;
use crate::loader::{Loader, LoaderError, LoaderType};
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::nes_frame::NesFrame;
use crate::nes_samples::NesSamples;
use crate::ppu::{PPU, PpuError, PpuNameTableMirroring, PpuType};
use crate::ppu_2c02::Ppu2c02;
use crate::ppu_dma::PpuDma;
use crate::sound_playback::SoundPlaybackError;
use crate::sound_playback_passive::SoundPlaybackPassive;
use crate::standard_controller::StandardController;

const WRAM_MEMORY_SIZE: usize = 2 * 1024;
const WRAM_START_ADDR: u16 = 0x0000;
const WRAM_END_ADDR: u16 = 0x1FFF;
const DEFAULT_START_ADDRESS: u16 = 0xFFFC;
const CYCLE_START_SEQUENCE: u32 = 7;
const CYCLE_CREDITS: u32 = 114;


pub struct NESConsole {
    cpu: Rc<RefCell<dyn CPU>>,
    ppu: Rc<RefCell<dyn PPU>>,
    apu: Rc<RefCell<dyn APU>>,
    controller: Rc<RefCell<dyn Controller>>,
    entry_point: Option<u16>,
    cycles_counter: u32,
    previous_cycles_counter: u32,
    cycles_debt: u32,
}

impl NESConsole {
    fn new(cpu: Rc<RefCell<dyn CPU>>,ppu: Rc<RefCell<dyn PPU>>, apu: Rc<RefCell<dyn APU>>, controller: Rc<RefCell<dyn Controller>>, entry_point: Option<u16>) -> NESConsole {
        NESConsole {
            cpu,
            ppu,
            apu,
            controller,
            entry_point,
            previous_cycles_counter: CYCLE_START_SEQUENCE,
            cycles_counter: CYCLE_START_SEQUENCE,
            cycles_debt: 0,
        }
    }

    pub fn set_input(&self, events: KeyEvents) -> Result<(), NESConsoleError>{
        self.controller.borrow_mut().set_input(events).map_err(|e|
            NESConsoleError::ControllerError(format!("{}", e.to_string())))
    }

    pub fn get_sample(&self) -> Result<Vec<f32>, NESConsoleError> {
        let vec = Vec::new();

        Ok(vec)
    }

    pub fn step_frame(&mut self) -> Result<(NesFrame, NesSamples), NESConsoleError> {
        let credits = CYCLE_CREDITS;
        let mut out_frame: Option<NesFrame> = None;
        let mut out_samples: NesSamples = NesSamples::default();

        loop {
            self.cycles_counter = self.cpu.borrow_mut().run(self.cycles_counter, credits - self.cycles_debt)?;
            self.cycles_debt = (self.cycles_counter - self.previous_cycles_counter) - (credits - self.cycles_debt);

            let (_, ppu_frame) = self.ppu.borrow_mut().run(self.cycles_counter, credits)?;
            if let Some(f) = ppu_frame {
                out_frame = Some(f);
            }

            let (_, apu_samples) = self.apu.borrow_mut().run(self.cycles_counter, credits)?;
            if let Some(s) = apu_samples {
                out_samples.append(s);
            }

            self.previous_cycles_counter = self.cycles_counter;

            if out_frame.is_some() {
                break;
            }
        };

        Ok((out_frame.unwrap(), out_samples))
    }

    fn reset_entry_point(&mut self) -> Result<(), NESConsoleError> {
        if let Some(pc) = self.entry_point {
            self.cpu.borrow_mut().set_pc_immediate(pc)?
        } else {
            self.cpu.borrow_mut().set_pc_indirect(DEFAULT_START_ADDRESS)?
        }

        Ok(())
    }

    pub fn power_on(&mut self) -> Result<(), NESConsoleError> {
        self.reset_entry_point()?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), NESConsoleError> {
        self.cpu.borrow_mut().reset()?;
        self.ppu.borrow_mut().reset()?;
        self.apu.borrow_mut().reset()?;

        self.reset_entry_point()?;

        Ok(())
    }
}

#[derive(Debug)]
pub enum NESConsoleError {
    BuilderError(String),
    IOError(String),
    ProgramLoaderError(String),
    CpuError(CpuError),
    PpuError(PpuError),
    ApuError(ApuError),
    InternalError(String),
    ControllerError(String),
    ChannelCommunication(String),
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
        NESConsoleError::CpuError(error)
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
        NESConsoleError::PpuError(error)
    }
}

impl From<ApuError> for NESConsoleError {
    fn from(error: ApuError) -> Self {
        NESConsoleError::ApuError(error)
    }
}

impl From<InputError> for NESConsoleError {
    fn from(error: InputError) -> Self {
        match error {
            InputError::InputFailure(s) => NESConsoleError::InternalError(s)
        }
    }
}

impl From<SoundPlaybackError> for NESConsoleError {
    fn from(error: SoundPlaybackError) -> Self {
        match error {
            SoundPlaybackError::SoundPlaybackFailure(s) => NESConsoleError::InternalError(s)
        }
    }
}


impl Display for NESConsoleError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            NESConsoleError::BuilderError(s) => { write!(f, "builder error: {}", s) },
            NESConsoleError::IOError(s) => { write!(f, "i/o error: {}", s) },
            NESConsoleError::ProgramLoaderError(s) => { write!(f, "program loader error: {}", s) },
            NESConsoleError::CpuError(s) => { write!(f, "cpu error: {}", s) },
            NESConsoleError::PpuError(s) => { write!(f, "ppu error: {}", s) },
            NESConsoleError::ApuError(s) => { write!(f, "apu error: {}", s) }
            NESConsoleError::InternalError(s) => { write!(f, "internal error: {}", s) }
            NESConsoleError::ControllerError(s) => { write!(f, "controller error: {}", s) }
            NESConsoleError::ChannelCommunication(s) => { write!(f, "channel communication error: {}", s) }
        }
    }
}

pub struct NESConsoleBuilder {
    cpu: Option<Rc<RefCell<dyn CPU>>>,
    cpu_type: Option<CpuType>,
    cpu_tracing: bool,
    cpu_trace_file: Option<File>,
    bus: Option<Rc<RefCell<dyn Bus>>>,
    bus_type: Option<BusType>,
    ppu: Option<Rc<RefCell<dyn PPU>>>,
    ppu_type: Option<PpuType>,
    apu: Option<Rc<RefCell<dyn APU>>>,
    apu_type: Option<ApuType>,
    controller: Option<Rc<RefCell<dyn Controller>>>,
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
            cpu_tracing: false,
            cpu_trace_file: None,
            bus: None,
            bus_type: None,
            ppu: None,
            ppu_type: None,
            apu: None,
            apu_type: None,
            controller: None,
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

    pub fn with_cpu_tracing_options(mut self, cpu: CpuType, trace: bool, trace_file: Option<File>) -> Self {
        self.cpu_type = Some(cpu);
        self.cpu_tracing = trace;
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

    fn build_cpu(&mut self, bus: Rc<RefCell<dyn Bus>>) -> Result<Rc<RefCell<dyn CPU>>, NESConsoleError> {
        debug!("creating cpu: {:?}", self.cpu_type.clone().unwrap());

        let result: Result<Rc<RefCell<dyn CPU>>, NESConsoleError> = match &self.cpu_type {
            Some(CpuType::NES6502) => {
                let mut cpu = Cpu6502::new(bus, self.cpu_tracing, self.cpu_trace_file.take());
                cpu.initialize()?;
                Ok(Rc::new(RefCell::new(cpu)))
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

    fn build_ppu_dma(&self, ppu_dma_type: &PpuDmaType, bus: Rc<RefCell<dyn Bus>>, ppu: Rc<RefCell<dyn DmaDevice>>) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError>{
        debug!("creating ppu dma {:?}", ppu_dma_type);

        let ppu_dma = match ppu_dma_type {
            PpuDmaType::NESPPUDMA => {
                PpuDma::new(ppu, bus)
            },
        };

        Ok(Rc::new(RefCell::new(ppu_dma)))
    }

    fn build_ppu_device(&mut self, ppu_type: &PpuType, chr_rom: Rc<RefCell<dyn BusDevice>>,
                        mirroring: PpuNameTableMirroring, bus: Rc<RefCell<dyn Bus>>,
                        cpu: Rc<RefCell<dyn CPU>>) -> Result<(Rc<RefCell<dyn BusDevice>>, Rc<RefCell<dyn BusDevice>>), NESConsoleError> {
        debug!("creating ppu {:?}", ppu_type);

        let result = match ppu_type {
            PpuType::NES2C02 => {
                Ppu2c02::new(chr_rom, mirroring, cpu)?
            },
        };

        let ppu = Rc::new(RefCell::new(result));
        let dma = self.build_ppu_dma(&PpuDmaType::NESPPUDMA, bus.clone(), ppu.clone())?;

        ppu.borrow_mut().initialize()?;
        dma.borrow_mut().initialize()?;

        self.ppu = Some(ppu.clone());
        self.ppu_type = Some(ppu_type.clone());

        Ok((ppu.clone(), dma))
    }

    fn build_controller_device(&self, controller_type: &ControllerType) -> Result<Rc<RefCell<dyn Controller>>, NESConsoleError> {
        debug!("creating controller {:?}", controller_type);

        let result = match controller_type {
            ControllerType::StandardController => {
                let input = InputExternal::new();
                StandardController::new(input)
            },
        };

        let controller = Rc::new(RefCell::new(result));
        controller.borrow_mut().initialize()?;

        Ok(controller)
    }

    fn build_apu_device(&mut self, apu_type: &ApuType, bus: Rc<RefCell<dyn Bus>>, cpu: Rc<RefCell<dyn CPU>>) -> Result<Rc<RefCell<dyn BusDevice>>, NESConsoleError> {
        debug!("creating apu {:?}", apu_type);

        let result = match apu_type {
            ApuType::RP2A03 => {
                let sound_player = SoundPlaybackPassive::new();
                ApuRp2A03::new(sound_player, cpu, bus)
            },
        };

        let apu = Rc::new(RefCell::new(result));
        apu.borrow_mut().initialize()?;

        self.apu = Some(apu.clone());
        self.apu_type = Some(apu_type.clone());

        Ok(apu)
    }

    fn build_cartridge_device(&self) -> Result<Rc<RefCell<dyn Cartridge>>, NESConsoleError> {
        debug!("creating cartridge");

        if let Some(ref rom_file) = self.rom_file {
            let path = PathBuf::from(rom_file);
            let loader = self.build_loader(path)?;
            let cartridge = loader.build_cartridge()?;

            Ok(cartridge)
        } else {
            Err(NESConsoleError::BuilderError("rom file not specified".to_string()))
        }
    }

    fn build_device_and_connect_to_bus(&mut self, device_type: &BusDeviceType,
                                       bus: Rc<RefCell<dyn Bus>>, cpu: Rc<RefCell<dyn CPU>>) -> Result<(), NESConsoleError> {
        debug!("creating device: {:?}", device_type);

        match device_type {
            BusDeviceType::CARTRIDGE(_) => {
                let cartridge = self.build_cartridge_device()?;
                bus.borrow_mut().add_device(cartridge.clone())?;
                self.cartridge = Some(cartridge.clone());
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

                let (ppu, dma) = self.build_ppu_device(ppu_type, chr_rom, mirroring, bus.clone(), cpu)?;
                bus.borrow_mut().add_device(ppu)?;
                bus.borrow_mut().add_device(dma)?;
            },

            BusDeviceType::CONTROLLER(controller_type) => {
                let controller = self.build_controller_device(controller_type)?;
                bus.borrow_mut().add_device(controller.clone())?;
                self.controller = Some(controller.clone());
            }

            BusDeviceType::APU(apu_type) => {
                let apu= self.build_apu_device(apu_type,bus.clone(), cpu)?;
                bus.borrow_mut().add_device(apu)?;
            }

            _ => {}
        };

        Ok(())
    }

    fn build_loader(&self, path: PathBuf) -> Result<impl Loader, NESConsoleError> {
        debug!("creating loader: {:?}", self.loader_type.clone().unwrap());

        match self.loader_type {
            None => {
                Err(NESConsoleError::BuilderError("loader not set".to_string()))
            },
            Some(LoaderType::INESV2) => {
                Ok(INesLoader::from_file(path)?)
            }
        }
    }

    fn build_nes(mut self) -> Result<NESConsole, NESConsoleError> {
        let bus = self.build_bus()?;
        let cpu = self.build_cpu(bus.clone())?;

        self.bus = Some(bus.clone());
        self.cpu = Some(cpu.clone());

        let device_types = self.device_types.clone();

        for device_type in device_types {
            self.build_device_and_connect_to_bus(&device_type, bus.clone(), cpu.clone())?;
        }

        let cpu = self.cpu.take()
            .ok_or(NESConsoleError::BuilderError("cpu missing".to_string()))?;

        let ppu = self.ppu.take()
            .ok_or(NESConsoleError::BuilderError("ppu missing".to_string()))?;

        let apu = self.apu.take()
            .ok_or(NESConsoleError::BuilderError("apu missing".to_string()))?;

        let controller = self.controller.take()
            .ok_or(NESConsoleError::BuilderError("controller missing".to_string()))?;

        let console = NESConsole::new(cpu, ppu, apu, controller, self.entry_point.take());

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