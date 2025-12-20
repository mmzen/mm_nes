// Authorship: Human 45% | Claude 55%
use std::cell::{Cell, RefCell};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::rc::Rc;
use log::{debug, info};
use crate::apu::{ApuError, ApuType, APU};
use crate::apu_rp2a03::ApuRp2A03;
use crate::bus::{Bus, BusError, BusType};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::config_spec::{ConfigSpec, Configurable};
use crate::controller::{Controller, ControllerType};
use crate::cpu::{CPU, CpuCycleResult, CpuError, CpuType};
use crate::cpu_6502::Cpu6502;
use crate::cpu_debugger::CpuSnapshot;
use crate::dma::PpuDmaType;
use crate::dma_controller::DmaController;
use crate::dma_device::DmaDevice;
use crate::ines_loader::INesLoader;
use crate::input::InputError;
use crate::input_external::InputExternal;
use crate::key_event::KeyEvents;
use crate::loader::{Loader, LoaderError, LoaderType};
use crate::memory::{Memory, MemoryError, MemoryType};
use crate::memory_bank::MemoryBank;
use crate::memory_ciram::PpuNameTableMirroring;
use crate::nes_bus::NESBus;
use crate::nes_frame::NesFrame;
use crate::nes_samples::NesSamples;
use crate::ppu::{PPU, PpuError, PpuType};
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

///
/// CPU cycle counter for CPU, APU and PPU
///
/// All cycles are converted to CPU cycles equivalents
///
struct CyclesCounter {
    current: u32,
    previous: u32,
    debt: u32,      // Reserved for future DMC DMA cycle stealing
    credits: u32,   // Threshold for catch-up synchronization
}

impl CyclesCounter {
    fn new(current: u32) -> CyclesCounter {
        CyclesCounter {
            current,
            previous: current,
            debt: 0,
            credits: 0,
        }
    }

    fn set_credits(&mut self, credits: u32) {
        self.credits = credits;
    }

    fn ahead(&self, other: &CyclesCounter, threshold: u32) -> bool {
        self.current.saturating_sub(other.current) >= threshold
    }
}

pub struct NesConsole {
    cpu: Rc<RefCell<dyn CPU>>,
    ppu: Rc<RefCell<dyn PPU>>,
    apu: Rc<RefCell<dyn APU>>,
    controller: Rc<RefCell<dyn Controller>>,
    entry_point: Option<u16>,
    cpu_counter: CyclesCounter,
    apu_counter: CyclesCounter,
    ppu_counter: CyclesCounter,
    config: ConfigSpec,
    /// Shared cell for OAM DMA start signal (page to transfer from)
    dma_start_page: Rc<Cell<Option<u8>>>,
    /// Shared cell tracking CPU odd/even cycle (for DMA alignment)
    cpu_cycle_odd: Rc<Cell<bool>>,
    /// Total master cycles executed (for cycle-accurate mode)
    master_cycles: u64,
    /// DMA controller for cycle-accurate OAM and DMC DMA
    dma_controller: DmaController<dyn Bus, dyn DmaDevice>,
}

impl Configurable for NesConsole {
    fn set_config(&mut self, config: ConfigSpec) {
        info!("setting configuration: {}", config.region);

        self.config = config.clone();

        let ppu_credits = self.compute_ppu_credits(&config);
        let apu_credits = self.compute_apu_credits(&config);

        info!("setting ppu credits: {}", ppu_credits);
        self.ppu_counter.set_credits(ppu_credits);

        info!("setting apu credits: {}", apu_credits);
        self.apu_counter.set_credits(apu_credits);

        self.ppu.borrow_mut().set_config(config.clone());
        self.apu.borrow_mut().set_config(config.clone());

        // self.cpu.borrow_mut().set_config(config.clone());
    }
}

impl NesConsole {
    
    pub fn config(&self) -> &ConfigSpec {
        &self.config
    }    
    
    /// Compute the number of CPU cycles that correspond to one scanline (341 PPU dots).
    /// Used as the threshold for PPU catch-up synchronization.
    fn compute_ppu_credits(&self, config: &ConfigSpec) -> u32 {
        let credits = (341.0 * (config.cpu_clock_hz / config.ppu_clock_hz)).floor() as u32;
        credits.max(1)
    }

    /// Compute the number of CPU cycles for APU catch-up synchronization.
    /// Uses the same threshold as PPU (one scanline worth of cycles).
    fn compute_apu_credits(&self, config: &ConfigSpec) -> u32 {
        self.compute_ppu_credits(config)
    }

    fn new(
        cpu: Rc<RefCell<dyn CPU>>,
        ppu: Rc<RefCell<dyn PPU>>,
        apu: Rc<RefCell<dyn APU>>,
        controller: Rc<RefCell<dyn Controller>>,
        entry_point: Option<u16>,
        config: ConfigSpec,
        dma_start_page: Rc<Cell<Option<u8>>>,
        cpu_cycle_odd: Rc<Cell<bool>>,
        dma_controller: DmaController<dyn Bus, dyn DmaDevice>,
    ) -> NesConsole {
        let mut console = NesConsole {
            cpu,
            ppu,
            apu,
            controller,
            entry_point,
            cpu_counter: CyclesCounter::new(CYCLE_START_SEQUENCE),
            apu_counter: CyclesCounter::new(0),
            // PPU counter starts at CYCLE_START_SEQUENCE to match CPU counter
            ppu_counter: CyclesCounter::new(CYCLE_START_SEQUENCE),
            config: config.clone(),
            dma_start_page,
            cpu_cycle_odd,
            master_cycles: 0,
            dma_controller,
        };

        console.set_config(config);

        // Synchronize PPU internal dot position with CPU's initial cycle offset
        let startup_dots = CYCLE_START_SEQUENCE * 3;
        let _ = console.ppu.borrow_mut().advance_dots(startup_dots);

        console
    }

    pub fn set_input(&self, events: KeyEvents) -> Result<(), NesConsoleError>{
        self.controller.borrow_mut().set_input(events).map_err(|e|
            NesConsoleError::ControllerError(format!("{}", e.to_string())))
    }

    pub fn get_sample(&self) -> Result<Vec<f32>, NesConsoleError> {
        let vec = Vec::new();

        Ok(vec)
    }

    /// Synchronize PPU and APU with the CPU.
    /// This method runs PPU/APU for the exact number of cycles the CPU is ahead,
    /// providing instruction-level synchronization for better accuracy.
    pub fn catch_up_ppu_and_apu(&mut self) -> Result<(Option<NesFrame>, Option<NesSamples>), NesConsoleError> {
        let mut out_frame: Option<NesFrame> = None;
        let mut out_samples: Option<NesSamples> = None;

        // Calculate how many CPU cycles the PPU needs to catch up
        let ppu_delta = self.cpu_counter.current.saturating_sub(self.ppu_counter.current);
        if ppu_delta > 0 {
            let (ppu_cycles, ppu_frame) = self.ppu.borrow_mut().run(self.ppu_counter.current, ppu_delta)?;
            if let Some(frame) = ppu_frame {
                out_frame = Some(frame);
            }
            self.ppu_counter.current = ppu_cycles;
            self.ppu_counter.previous = self.ppu_counter.current;
        }

        // Calculate how many CPU cycles the APU needs to catch up
        let apu_delta = self.cpu_counter.current.saturating_sub(self.apu_counter.current);
        if apu_delta > 0 {
            let (apu_cycles, apu_samples) = self.apu.borrow_mut().run(self.apu_counter.current, apu_delta)?;
            if let Some(sample) = apu_samples {
                if let Some(acc) = &mut out_samples {
                    acc.append(sample);
                } else {
                    out_samples = Some(sample);
                }
            }
            self.apu_counter.current = apu_cycles;
            self.apu_counter.previous = self.apu_counter.current;
        }

        // Clear any DMC stall cycles (not applied - would cause audio desync)
        let _ = self.apu.borrow_mut().get_dmc_stall_cycles();

        Ok((out_frame, out_samples))
    }

    /// Execute a single master cycle (one CPU cycle).
    /// This is the cycle-accurate stepping method that advances all components together.
    ///
    /// Returns:
    /// - `CpuCycleResult`: Information about what the CPU did this cycle
    /// - `Option<NesFrame>`: A completed frame if PPU finished rendering
    /// - `Option<NesSamples>`: Audio samples generated
    pub fn step_master_cycle(&mut self) -> Result<(CpuCycleResult, Option<NesFrame>, Option<NesSamples>), NesConsoleError> {
        let mut out_frame: Option<NesFrame> = None;
        let mut out_samples: Option<NesSamples> = None;

        // Check if OAM DMA was triggered this cycle
        if let Some(page) = self.dma_start_page.get() {
            self.dma_start_page.set(None);
            self.dma_controller.set_cpu_cycle_odd(self.cpu_cycle_odd.get());
            self.dma_controller.start_oam_dma(page);
        }

        // Check if DMC DMA is needed (before executing CPU cycle)
        // DMC DMA can steal cycles mid-instruction, so we check every cycle
        if !self.dma_controller.is_dmc_dma_active() {
            if let Some(dmc_address) = self.apu.borrow().needs_dmc_dma() {
                // Calculate how many cycles to steal based on current CPU state
                let cpu_is_halted = self.dma_controller.is_active() || self.cpu.borrow().is_halted();
                let oam_dma_active = self.dma_controller.is_oam_dma_active();
                // For writes, we need to wait longer. We'll determine this from the previous cycle's activity.
                // Since we're checking BEFORE the cycle, we use false for writes (conservative approach).
                // The real 2A03 checks what the CPU is about to do, but we don't have that info yet.
                let cpu_is_writing = false; // Will be refined if needed

                let dmc_cycles = DmaController::<dyn Bus, dyn DmaDevice>::calculate_dmc_dma_cycles(
                    cpu_is_writing,
                    cpu_is_halted,
                    oam_dma_active,
                );
                self.dma_controller.start_dmc_dma(dmc_address, dmc_cycles);
            }
        }

        // Execute one cycle: either DMA or CPU
        let (cpu_result, dmc_sample) = if self.dma_controller.is_active() {
            // DMA is active - step the DMA controller, CPU is halted
            let dma_result = self.dma_controller.step_cycle()
                .map_err(|e| NesConsoleError::InternalError(format!("DMA error: {}", e)))?;

            let result = CpuCycleResult {
                halted: true,
                instruction_complete: false,
                memory_read: dma_result.read_occurred,
                memory_write: dma_result.write_occurred,
                address: dma_result.address_accessed,
                ..Default::default()
            };

            (result, dma_result.dmc_dma_complete)
        } else {
            // Normal CPU execution
            (self.cpu.borrow_mut().step_cycle()?, None)
        };

        // Deliver DMC sample to APU if DMC DMA completed
        if let Some(sample) = dmc_sample {
            self.apu.borrow_mut().provide_dmc_sample(sample)
                .map_err(|e| NesConsoleError::ApuError(e))?;
        }

        // Calculate total CPU cycles this step (1 base + any interrupt cycles)
        let total_cpu_cycles = 1 + cpu_result.interrupt_cycles;

        // Update cycle counters
        self.cpu_counter.current += total_cpu_cycles;
        self.master_cycles += total_cpu_cycles as u64;

        // Toggle CPU cycle parity (for DMA alignment tracking)
        // Parity changes once per cycle, so after N cycles it changes by (N mod 2)
        // Only toggle if odd number of cycles consumed
        if total_cpu_cycles % 2 == 1 {
            let new_parity = !self.cpu_cycle_odd.get();
            self.cpu_cycle_odd.set(new_parity);
        }
        self.dma_controller.set_cpu_cycle_odd(self.cpu_cycle_odd.get());

        // Advance PPU using the same run() method as instruction-level mode
        // This converts CPU cycles to PPU dots internally
        let (new_ppu_cycles, ppu_frame) = self.ppu.borrow_mut().run(self.ppu_counter.current, total_cpu_cycles)?;
        if let Some(frame) = ppu_frame {
            out_frame = Some(frame);
        }
        self.ppu_counter.current = new_ppu_cycles;

        // Advance APU by total CPU cycles
        let (apu_cycles, apu_samples) = self.apu.borrow_mut().run(self.apu_counter.current, total_cpu_cycles)?;
        if let Some(samples) = apu_samples {
            out_samples = Some(samples);
        }
        self.apu_counter.current = apu_cycles;

        Ok((cpu_result, out_frame, out_samples))
    }

    /// Execute cycles until a complete frame is rendered (cycle-accurate mode).
    /// This provides more accurate emulation than `step_frame()` at the cost of performance.
    pub fn step_frame_cycle_accurate(&mut self) -> Result<(NesFrame, NesSamples), NesConsoleError> {
        let mut out_samples = NesSamples::default();

        loop {
            let (_, frame, samples) = self.step_master_cycle()?;

            if let Some(s) = samples {
                out_samples.append(s);
            }

            if let Some(frame) = frame {
                return Ok((frame, out_samples));
            }
        }
    }

    ///
    /// Execute a single CPU instruction and:  
    ///     - catch up the PPU and APU if necessary  
    ///     - fetch a CPU snapshot  
    ///     - returns an optional frame, an optional sound samples buffer, and a CPU snapshot  
    /// 
    pub fn step_instruction(&mut self) -> Result<(Option<NesFrame>, Option<NesSamples>, Box<dyn CpuSnapshot>), NesConsoleError> {

        self.cpu_counter.current += self.cpu.borrow_mut().step_instruction()?;
        let snapshot = self.cpu.borrow().snapshot()?;

        // Handle OAM DMA in instruction-level mode
        // When the CPU writes to $4014, dma_start_page is set. We need to add the
        // DMA transfer cycles (513-514) to maintain correct PPU/CPU synchronization.
        if self.dma_start_page.get().is_some() {
            self.dma_start_page.set(None);
            // OAM DMA takes 513 cycles (even start) or 514 cycles (odd start)
            // Since we don't track cycle parity in instruction-level mode, use 513
            // which is the minimum. The actual transfer is handled by PpuDma device.
            const OAM_DMA_CYCLES: u32 = 513;
            self.cpu_counter.current += OAM_DMA_CYCLES;
        }

        let (out_frame ,out_samples) = self.catch_up_ppu_and_apu()?;

        self.cpu_counter.previous = self.cpu_counter.current;

        Ok((out_frame, out_samples, snapshot))
    }
    
    pub fn step_frame_debug(&mut self) -> Result<(NesFrame, NesSamples, Vec<Box<dyn CpuSnapshot>>), NesConsoleError> {
        let out_frame: Option<NesFrame>;
        let mut out_samples: NesSamples = NesSamples::default();
        let mut snapshots: Vec<Box<dyn CpuSnapshot>> = Vec::new();

        loop {
            let (frame, samples, snapshot) = self.step_instruction()?;
            snapshots.push(snapshot);
            
            if let Some(s) = samples {
                out_samples.append(s);
            }

            if frame.is_some() {
                out_frame = frame;
                break;
            }
        };

        Ok((out_frame.unwrap(), out_samples, snapshots))
    }

    pub fn step_frame(&mut self) -> Result<(NesFrame, NesSamples), NesConsoleError> {
        let out_frame: Option<NesFrame>;
        let mut out_samples: NesSamples = NesSamples::default();

        loop {
            let (frame, samples, snapshot) = self.step_instruction()?;

            if let Some(s) = samples {
                out_samples.append(s);
            }

            if frame.is_some() {
                out_frame = frame;
                break;
            }
        };

        Ok((out_frame.unwrap(), out_samples))
    }

    fn reset_entry_point(&mut self) -> Result<(), NesConsoleError> {
        if let Some(pc) = self.entry_point {
            self.cpu.borrow_mut().set_pc_immediate(pc)?
        } else {
            self.cpu.borrow_mut().set_pc_indirect(DEFAULT_START_ADDRESS)?
        }

        Ok(())
    }

    pub fn power_on(&mut self) -> Result<(), NesConsoleError> {
        self.reset_entry_point()?;
        Ok(())
    }

    fn reset_counters(&mut self) {
        self.cpu_counter = CyclesCounter::new(CYCLE_START_SEQUENCE);
        self.apu_counter = CyclesCounter::new(0);
        // PPU counter starts at CYCLE_START_SEQUENCE to match CPU counter.
        // This ensures both instruction-level and cycle-accurate modes are synchronized.
        self.ppu_counter = CyclesCounter::new(CYCLE_START_SEQUENCE);
        self.dma_start_page.set(None);
        self.cpu_cycle_odd.set(false);
        self.master_cycles = 0;
        self.dma_controller.reset();
    }

    /// Synchronize PPU internal dot position with the CPU's initial cycle offset.
    /// The CPU starts at cycle 7 (reset sequence), so the PPU should be at dot 21 (7 * 3).
    /// This must be called after reset_counters() and PPU reset.
    fn sync_ppu_startup(&mut self) -> Result<(), NesConsoleError> {
        // Advance PPU by the dots corresponding to CPU's initial cycle offset
        let startup_dots = CYCLE_START_SEQUENCE * 3;
        self.ppu.borrow_mut().advance_dots(startup_dots)?;
        Ok(())
    }

    pub fn reset(&mut self) -> Result<(), NesConsoleError> {
        self.cpu.borrow_mut().reset()?;
        self.ppu.borrow_mut().reset()?;
        self.apu.borrow_mut().reset()?;

        self.reset_counters();
        self.reset_entry_point()?;
        // set_config must be called before sync_ppu_startup because
        // set_config resets PPU dot position, which sync_ppu_startup then adjusts
        self.set_config(self.config.clone());
        self.sync_ppu_startup()?;

        Ok(())
    }
}

#[derive(Debug, Clone)]
pub enum NesConsoleError {
    BuilderError(String),
    IOError(String),
    ProgramLoaderError(String),
    CpuError(CpuError),
    PpuError(PpuError),
    ApuError(ApuError),
    InternalError(String),
    ControllerError(String),
    ChannelCommunication(String),
    Terminated(String)
}

impl From<std::io::Error> for NesConsoleError {
    fn from(error: std::io::Error) -> Self {
        NesConsoleError::IOError(error.to_string())
    }
}

impl From<MemoryError> for NesConsoleError {
    fn from(error: MemoryError) -> Self {
        NesConsoleError::IOError(error.to_string())
    }
}

impl From<CpuError> for NesConsoleError {
    fn from(error: CpuError) -> Self {
        NesConsoleError::CpuError(error)
    }
}

impl From<BusError> for NesConsoleError {
    fn from(error: BusError) -> Self {
        NesConsoleError::BuilderError(error.to_string())
    }
}

impl From<LoaderError> for NesConsoleError {
    fn from(error: LoaderError) -> Self {
        NesConsoleError::ProgramLoaderError(error.to_string())
    }
}

impl From<PpuError> for NesConsoleError {
    fn from(error: PpuError) -> Self {
        NesConsoleError::PpuError(error)
    }
}

impl From<ApuError> for NesConsoleError {
    fn from(error: ApuError) -> Self {
        NesConsoleError::ApuError(error)
    }
}

impl From<InputError> for NesConsoleError {
    fn from(error: InputError) -> Self {
        match error {
            InputError::InputFailure(s) => NesConsoleError::InternalError(s)
        }
    }
}

impl From<SoundPlaybackError> for NesConsoleError {
    fn from(error: SoundPlaybackError) -> Self {
        match error {
            SoundPlaybackError::SoundPlaybackFailure(s) => NesConsoleError::InternalError(s)
        }
    }
}


impl Display for NesConsoleError {
    fn fmt(&self, f: &mut Formatter) -> std::fmt::Result {
        match self {
            NesConsoleError::BuilderError(s) => { write!(f, "builder error: {}", s) },
            NesConsoleError::IOError(s) => { write!(f, "i/o error: {}", s) },
            NesConsoleError::ProgramLoaderError(s) => { write!(f, "program loader error: {}", s) },
            NesConsoleError::CpuError(s) => { write!(f, "cpu error: {}", s) },
            NesConsoleError::PpuError(s) => { write!(f, "ppu error: {}", s) },
            NesConsoleError::ApuError(s) => { write!(f, "apu error: {}", s) }
            NesConsoleError::InternalError(s) => { write!(f, "internal error: {}", s) }
            NesConsoleError::ControllerError(s) => { write!(f, "controller error: {}", s) }
            NesConsoleError::ChannelCommunication(s) => { write!(f, "channel communication error: {}", s) }
            NesConsoleError::Terminated(s) => {write!(f, "emulator terminated: {}", s) }
        }
    }
}

pub struct NesConsoleBuilder {
    cpu: Option<Rc<RefCell<dyn CPU>>>,
    cpu_type: Option<CpuType>,
    bus: Option<Rc<RefCell<dyn Bus>>>,
    bus_type: Option<BusType>,
    ppu: Option<Rc<RefCell<dyn PPU>>>,
    ppu_type: Option<PpuType>,
    apu: Option<Rc<RefCell<dyn APU>>>,
    apu_type: Option<ApuType>,
    controller: Option<Rc<RefCell<dyn Controller>>>,
    device_types: Vec<BusDeviceType>,
    loader_type: Option<LoaderType>,
    rom_file: Option<PathBuf>,
    entry_point: Option<u16>,
    cartridge: Option<Rc<RefCell<dyn Cartridge>>>,
    config: ConfigSpec,
    /// Shared cell for OAM DMA start signal (page to transfer from)
    dma_start_page: Rc<Cell<Option<u8>>>,
    /// Shared cell for CPU cycle parity tracking
    cpu_cycle_odd: Rc<Cell<bool>>,
    /// Reference to PPU as DmaDevice for DmaController
    ppu_dma_device: Option<Rc<RefCell<dyn DmaDevice>>>,
}

impl NesConsoleBuilder {
    pub fn new() -> Self {
        NesConsoleBuilder {
            cpu: None,
            cpu_type: None,
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
            config: ConfigSpec::default(),
            dma_start_page: Rc::new(Cell::new(None)),
            cpu_cycle_odd: Rc::new(Cell::new(false)),
            ppu_dma_device: None,
        }
    }


    pub fn with_cpu(mut self, cpu: CpuType) -> Self {
        self.cpu_type = Some(cpu);
        self
    }

    pub fn with_loader_type(mut self, loader_type: LoaderType) -> Self {
        self.loader_type = Some(loader_type);
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

    pub fn with_rom_file(mut self, rom_file: PathBuf) -> Self {
        debug!("setting rom file: {:?}", rom_file);

        self.rom_file = Some(rom_file);
        self
    }

    pub fn with_entry_point(mut self, entry_point: Option<u16>) -> Self {
        self.entry_point = entry_point;
        self
    }

    fn build_cpu(&mut self, bus: Rc<RefCell<dyn Bus>>) -> Result<Rc<RefCell<dyn CPU>>, NesConsoleError> {
        debug!("creating cpu: {:?}", self.cpu_type.clone().unwrap());

        let result: Result<Rc<RefCell<dyn CPU>>, NesConsoleError> = match &self.cpu_type {
            Some(CpuType::NES6502) => {
                let mut cpu = Cpu6502::new(bus);
                cpu.initialize()?;
                Ok(Rc::new(RefCell::new(cpu)))
            },

            None => {
                Err(NesConsoleError::BuilderError("CPU type not specified".to_string()))
            }
        };

        result
    }

    fn build_bus(&self) -> Result<(Rc<RefCell<dyn Bus>>, Rc<Cell<u8>>), NesConsoleError> {
        debug!("creating bus: {:?}", self.bus_type.clone().unwrap());

        match self.bus_type {
            Some(BusType::NESBus) => {
                let bus = NESBus::new();
                let data_bus = bus.get_data_bus();
                Ok((Rc::new(RefCell::new(bus)), data_bus))
            },

            None => {
                Err(NesConsoleError::BuilderError("bus type not specified".to_string()))
            }
        }
    }

    fn build_wram_device(&self, memory_type: &MemoryType) -> Result<Rc<RefCell<dyn BusDevice>>, NesConsoleError> {
        debug!("creating wram: {:?}", memory_type);

        let mut wram = match memory_type {
            MemoryType::StandardMemory => {
                MemoryBank::new(WRAM_MEMORY_SIZE, (WRAM_START_ADDR, WRAM_END_ADDR))
            }
            _ => Err(NesConsoleError::BuilderError("invalid wram type specified".to_string()))?
        };

        wram.initialize()?;
        Ok(Rc::new(RefCell::new(wram)))
    }

    fn build_ppu_dma(&self, ppu_dma_type: &PpuDmaType, _bus: Rc<RefCell<dyn Bus>>, _ppu: Rc<RefCell<dyn DmaDevice>>) -> Result<Rc<RefCell<dyn BusDevice>>, NesConsoleError>{
        debug!("creating ppu dma {:?}", ppu_dma_type);

        let ppu_dma = match ppu_dma_type {
            PpuDmaType::NESPPUDMA => {
                // PpuDma now just signals OAM DMA start via shared cell,
                // actual transfer is handled by DmaController in the scheduler
                PpuDma::new_with_dma_signal(self.dma_start_page.clone())
            },
        };

        Ok(Rc::new(RefCell::new(ppu_dma)))
    }

    fn build_ppu_device(&mut self, ppu_type: &PpuType, chr_rom: Rc<RefCell<dyn BusDevice>>,
                        mirroring: Rc<RefCell<PpuNameTableMirroring>>, bus: Rc<RefCell<dyn Bus>>,
                        cpu: Rc<RefCell<dyn CPU>>, config: ConfigSpec) -> Result<(Rc<RefCell<dyn BusDevice>>, Rc<RefCell<dyn BusDevice>>), NesConsoleError> {
        debug!("creating ppu {:?}", ppu_type);

        let result = match ppu_type {
            PpuType::NES2C02 => {
                Ppu2c02::new(chr_rom, mirroring, cpu, config)?
            },
        };

        let ppu = Rc::new(RefCell::new(result));
        let dma = self.build_ppu_dma(&PpuDmaType::NESPPUDMA, bus.clone(), ppu.clone())?;

        ppu.borrow_mut().initialize()?;
        dma.borrow_mut().initialize()?;

        self.ppu = Some(ppu.clone());
        self.ppu_type = Some(ppu_type.clone());
        // Store PPU as DmaDevice for DmaController
        self.ppu_dma_device = Some(ppu.clone());

        Ok((ppu.clone(), dma))
    }

    fn build_controller_device(&self, controller_type: &ControllerType, data_bus: Rc<Cell<u8>>) -> Result<Rc<RefCell<dyn Controller>>, NesConsoleError> {
        debug!("creating controller {:?}", controller_type);

        let result = match controller_type {
            ControllerType::StandardController => {
                let input = InputExternal::new();
                StandardController::new(input, data_bus)
            },
        };

        let controller = Rc::new(RefCell::new(result));
        controller.borrow_mut().initialize()?;

        Ok(controller)
    }

    fn build_apu_device(&mut self, apu_type: &ApuType, bus: Rc<RefCell<dyn Bus>>, cpu: Rc<RefCell<dyn CPU>>, config: ConfigSpec, data_bus: Rc<Cell<u8>>) -> Result<Rc<RefCell<dyn BusDevice>>, NesConsoleError> {
        debug!("creating apu {:?}", apu_type);

        let result = match apu_type {
            ApuType::RP2A03 => {
                let sound_player = SoundPlaybackPassive::new();
                ApuRp2A03::new(sound_player, cpu, bus, config, data_bus)
            },
        };

        let apu = Rc::new(RefCell::new(result));
        apu.borrow_mut().initialize()?;

        self.apu = Some(apu.clone());
        self.apu_type = Some(apu_type.clone());

        Ok(apu)
    }

    fn build_cartridge_device(&self) -> Result<Rc<RefCell<dyn Cartridge>>, NesConsoleError> {
        debug!("creating cartridge");

        if let Some(ref rom_file) = self.rom_file {
            let loader = self.build_loader(rom_file.clone())?;
            let cartridge = loader.build_cartridge()?;

            Ok(cartridge)
        } else {
            Err(NesConsoleError::BuilderError("rom file not specified".to_string()))
        }
    }

    fn build_device_and_connect_to_bus(&mut self, device_type: &BusDeviceType,
                                       bus: Rc<RefCell<dyn Bus>>, cpu: Rc<RefCell<dyn CPU>>, data_bus: Rc<Cell<u8>>) -> Result<(), NesConsoleError> {
        debug!("creating device: {:?}", device_type);

        match device_type {
            /***
             * this needs to be created first as it contains cartridge-specific settings (region, nametable layout, etc ...)
             ***/
            BusDeviceType::CARTRIDGE(_) => {
                let cartridge = self.build_cartridge_device()?;
                let prg_ram = cartridge.borrow().get_prg_ram();
                if let Some(prg_ram) = prg_ram {
                    debug!("adding prg_ram: {} kb", prg_ram.borrow().size());
                    bus.borrow_mut().add_device(prg_ram)?;
                }
                bus.borrow_mut().add_device(cartridge.clone())?;

                let region = cartridge.borrow().get_region();
                self.config = ConfigSpec::from_region(region);
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
                    .ok_or(NesConsoleError::BuilderError("no cartridge to load".to_string()))?;

                let mirroring = self
                    .cartridge
                    .as_ref()
                    .map(|cartridge| cartridge.borrow().get_mirroring())
                    .ok_or(NesConsoleError::BuilderError("ppu mirroring not set".to_string()))?;

                let (ppu, dma) = self.build_ppu_device(ppu_type, chr_rom, mirroring, bus.clone(), cpu, self.config.clone())?;

                bus.borrow_mut().add_device(ppu)?;
                bus.borrow_mut().add_device(dma)?;
            },

            BusDeviceType::CONTROLLER(controller_type) => {
                let controller = self.build_controller_device(controller_type, data_bus.clone())?;
                bus.borrow_mut().add_device(controller.clone())?;
                self.controller = Some(controller.clone());
            }

            BusDeviceType::APU(apu_type) => {
                let apu = self.build_apu_device(apu_type, bus.clone(), cpu, self.config.clone(), data_bus)?;
                bus.borrow_mut().add_device(apu)?;
            }

            _ => {}
        };

        Ok(())
    }

    fn build_loader(&self, path: PathBuf) -> Result<impl Loader, NesConsoleError> {
        debug!("creating loader: {:?}", self.loader_type.clone().unwrap());

        match self.loader_type {
            None => {
                Err(NesConsoleError::BuilderError("loader not set".to_string()))
            },
            Some(LoaderType::INESV2) => {
                Ok(INesLoader::from_file(path)?)
            }
        }
    }

    fn build_nes(mut self) -> Result<NesConsole, NesConsoleError> {
        let (bus, data_bus) = self.build_bus()?;
        let cpu = self.build_cpu(bus.clone())?;

        self.bus = Some(bus.clone());
        self.cpu = Some(cpu.clone());

        let device_types = self.device_types.clone();

        for device_type in device_types {
            self.build_device_and_connect_to_bus(&device_type, bus.clone(), cpu.clone(), data_bus.clone())?;
        }

        let cpu = self.cpu.take()
            .ok_or(NesConsoleError::BuilderError("cpu missing".to_string()))?;

        let ppu = self.ppu.take()
            .ok_or(NesConsoleError::BuilderError("ppu missing".to_string()))?;

        let apu = self.apu.take()
            .ok_or(NesConsoleError::BuilderError("apu missing".to_string()))?;

        let controller = self.controller.take()
            .ok_or(NesConsoleError::BuilderError("controller missing".to_string()))?;

        // Create DmaController with bus and PPU (as DmaDevice)
        let ppu_dma_device = self.ppu_dma_device.take()
            .ok_or(NesConsoleError::BuilderError("ppu_dma_device missing".to_string()))?;
        let dma_controller = DmaController::new(bus, ppu_dma_device);

        let console = NesConsole::new(
            cpu,
            ppu,
            apu,
            controller,
            self.entry_point.take(),
            self.config,
            self.dma_start_page,
            self.cpu_cycle_odd,
            dma_controller,
        );

        Ok(console)
    }

    pub fn build(self) -> Result<NesConsole, NesConsoleError> {
        if let (Some(_), Some(_), Some(_), Some(_)) = (&self.bus_type, &self.cpu_type, &self.loader_type, &self.rom_file) {
            self.build_nes()
        } else {
            Err(NesConsoleError::BuilderError("missing required components".to_string()))
        }
    }
}