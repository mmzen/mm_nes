// Authorship: Human 18% | Claude 82%
use std::cell::{Cell, RefCell};
use std::fmt::{Display, Formatter};
use std::path::PathBuf;
use std::rc::Rc;
use log::{debug, info, trace};
use crate::apu::{ApuError, ApuType, APU};
use crate::apu_rp2a03::ApuRp2A03;
use crate::bus::{Bus, BusError, BusType};
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cartridge::Cartridge;
use crate::config_spec::{ConfigSpec, Configurable};
use crate::controller::{Controller, ControllerType};
use crate::cpu::{CPU, CpuCycleResult, CpuError, CpuType};
use crate::cpu_6502::{Cpu6502, MAPPER_IRQ};
use crate::cpu_debugger::CpuSnapshot;
use crate::dma::PpuDmaType;
use crate::dma_controller::{ApuPhase, DmaController};
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

/// Counter for tracking elapsed clock cycles.
/// Used for synchronizing component timing (CPU, APU, PPU).
/// All cycles are in CPU cycle units.
struct CyclesCounter {
    current: u32,
}

impl CyclesCounter {
    fn new(current: u32) -> CyclesCounter {
        CyclesCounter { current }
    }
}

pub struct NesConsole {
    cpu: Rc<RefCell<dyn CPU>>,
    ppu: Rc<RefCell<dyn PPU>>,
    apu: Rc<RefCell<dyn APU>>,
    controller: Rc<RefCell<dyn Controller>>,
    entry_point: Option<u16>,
    /// Master clock counter in CPU cycle units.
    /// Increments every cycle, including during DMA (when CPU is halted).
    /// This is the master timing reference, not "CPU execution cycles".
    cpu_counter: CyclesCounter,
    /// APU clock counter for tracking APU advancement
    apu_counter: CyclesCounter,
    /// PPU clock counter for tracking PPU advancement
    ppu_counter: CyclesCounter,
    config: ConfigSpec,
    /// Shared cell for OAM DMA start signal (page to transfer from)
    dma_start_page: Rc<Cell<Option<u8>>>,
    /// Total elapsed master cycles (u64 for long-running sessions).
    /// Same semantics as cpu_counter.current - increments every cycle including DMA.
    master_cycles: u64,
    /// DMA controller for cycle-accurate OAM DMA
    dma_controller: DmaController<dyn Bus>,
    /// Last address the CPU accessed on the bus (for DMA repeated reads)
    /// During DMA no-bus cycles, the external bus shows repeated reads from this address.
    /// This tracks ALL CPU bus operations (read OR write), not just reads.
    /// The real NES bus shows "repeated reads" from the last accessed address when
    /// the CPU is stalled, causing side effects for certain registers ($2002, $2007, etc.)
    last_cpu_bus_address: Option<u16>,
    /// Phase offset for GET/PUT cycle parity (0 or 1).
    ///
    /// The actual phase is DERIVED from `master_cycles + phase_offset`:
    /// - Even = GET phase (reads)
    /// - Odd = PUT phase (writes)
    ///
    /// This eliminates mutable toggle drift - phase is always a derived invariant.
    /// Randomized on power-on (not reset) to simulate real hardware indeterminism.
    phase_offset: u64,
    /// Cartridge reference for mapper IRQ polling (MMC3 scanline counter, etc.)
    cartridge: Option<Rc<RefCell<dyn Cartridge>>>,
}

impl Configurable for NesConsole {
    fn set_config(&mut self, config: ConfigSpec) {
        info!("setting configuration: {}", config.region);

        self.config = config.clone();

        self.ppu.borrow_mut().set_config(config.clone());
        self.apu.borrow_mut().set_config(config.clone());
    }
}

impl NesConsole {

    pub fn config(&self) -> &ConfigSpec {
        &self.config
    }

    fn new(
        cpu: Rc<RefCell<dyn CPU>>,
        ppu: Rc<RefCell<dyn PPU>>,
        apu: Rc<RefCell<dyn APU>>,
        controller: Rc<RefCell<dyn Controller>>,
        entry_point: Option<u16>,
        config: ConfigSpec,
        dma_start_page: Rc<Cell<Option<u8>>>,
        dma_controller: DmaController<dyn Bus>,
        cartridge: Option<Rc<RefCell<dyn Cartridge>>>,
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
            master_cycles: 0,
            dma_controller,
            last_cpu_bus_address: None,
            phase_offset: 0, // Will be randomized on power_on()
            cartridge,
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

    /// Execute a single master cycle (one CPU cycle).
    /// This is the cycle-accurate stepping method that advances all components together.
    ///
    /// The correct order of operations is:
    /// 1. Query CPU bus intent for this cycle (before execution)
    /// 2. Query APU for DMC DMA needs from its CURRENT state (before tick)
    /// 3. Check OAM DMA trigger
    /// 4. Decide bus master (CPU / OAM DMA / DMC DMA)
    /// 5. Execute bus operation for the selected master
    /// 6. Advance all components (APU, PPU)
    ///
    /// Returns:
    /// - `CpuCycleResult`: Information about what the CPU did this cycle
    /// - `Option<NesFrame>`: A completed frame if PPU finished rendering
    /// - `Option<NesSamples>`: Audio samples generated
    pub fn step_master_cycle(&mut self) -> Result<(CpuCycleResult, Option<NesFrame>, Option<NesSamples>), NesConsoleError> {
        let mut out_frame: Option<NesFrame> = None;
        let mut out_samples: Option<NesSamples> = None;

        // ============================================================
        // STEP 1: QUERY STATE BEFORE ANY COMPONENT ADVANCES
        // ============================================================

        // Query CPU bus intent for this cycle - is it about to read or write?
        // This is critical: DMA can only halt CPU on a READ cycle, not write.
        let cpu_bus_intent = self.cpu.borrow().get_pending_bus_operation();
        let cpu_is_writing = cpu_bus_intent.is_write;

        // Capture the PENDING read address for this cycle (not historical last read).
        // When DMA halts the CPU, repeated reads use the address of the read being halted,
        // not some previous read. This is critical for $2007 VRAM increment side effects.
        //
        // IMPORTANT: CPU bus intent may return None for certain cycles (complex addressing
        // modes, halted state). We MUST fall back to last_cpu_bus_address to ensure we
        // always have an address when halting. Without this, "one bus op per cycle" degenerates
        // into "sometimes zero" when stalled with None address.
        let pending_read_addr = if !cpu_is_writing {
            cpu_bus_intent.address.or(self.last_cpu_bus_address)
        } else {
            // CPU is writing - fall back to last bus address for repeated reads
            self.last_cpu_bus_address
        };

        // Query APU for DMC DMA needs from its CURRENT state (BEFORE ticking APU)
        //
        // DMC SCHEDULING CONTRACT:
        // - APU is AUTHORITATIVE for DMC timing. It returns Some(address) from needs_dmc_dma()
        //   only when the DMC sample buffer is empty and a fetch should begin THIS cycle.
        // - DMA controller is a DUMB EXECUTOR. It only runs the halt/dummy/align/read sequence.
        // - The APU handles load vs reload scheduling internally - the DMA controller does not
        //   need to know whether this is an initial load or a reload request.
        // - This separation ensures timing logic lives in one place (APU) and is testable there.
        //
        // NOTE: We let APU be fully authoritative - the DMA controller will ignore duplicate
        // requests if it's already processing a DMC fetch. No guard needed here.
        let dmc_dma_request = self.apu.borrow().needs_dmc_dma();

        // Check if OAM DMA was triggered (from write to $4014 last cycle)
        let oam_dma_start = self.dma_start_page.get();
        if oam_dma_start.is_some() {
            self.dma_start_page.set(None);
        }

        // ============================================================
        // STEP 2: DECIDE BUS MASTER AND START DMA IF NEEDED
        // ============================================================

        // Derive current phase from master_cycles (invariant - no mutable toggle drift)
        // Phase is computed BEFORE incrementing master_cycles, so it represents this cycle's phase.
        let current_phase = ApuPhase::from_cycle(self.master_cycles + self.phase_offset);

        // Start OAM DMA if triggered
        // NOTE: We do NOT set the halted read address here - it will be captured
        // by the DMA controller when halt actually succeeds (PendingHalt → Halt)
        //
        // ALIGNMENT: The $4014 write happened on the PREVIOUS cycle (master_cycles - 1).
        // The trigger phase was `current_phase.toggle()` (the phase before this one).
        // OAM DMA alignment (513 vs 514 cycles) depends on this trigger phase:
        // - Trigger on GET: First DMA step on PUT → halt on PUT → first read on GET → 513 total
        // - Trigger on PUT: First DMA step on GET → halt on GET → alignment cycle → 514 total
        // This alignment happens naturally via phase-gated state machine in DMA controller.
        if let Some(page) = oam_dma_start {
            // The trigger phase is deterministic from cycle count (Phase 1 fix).
            // No need to store it - derived from (master_cycles - 1 + phase_offset).
            let _trigger_phase = ApuPhase::from_cycle(self.master_cycles.wrapping_sub(1) + self.phase_offset);
            debug_assert_eq!(
                _trigger_phase.toggle(), current_phase,
                "Phase invariant violated: trigger phase {} should toggle to current phase {}",
                if _trigger_phase.is_get() { "GET" } else { "PUT" },
                if current_phase.is_get() { "GET" } else { "PUT" }
            );
            self.dma_controller.start_oam_dma(page);
        }

        // Start DMC DMA if APU requested it
        // NOTE: Same as above - address captured at halt, not at request time
        if let Some(dmc_address) = dmc_dma_request {
            let accepted = self.dma_controller.request_dmc_dma(dmc_address);
            if !accepted {
                trace!("DMC DMA request rejected (already active) at address ${:04X}", dmc_address);
            }
        }

        // ============================================================
        // SUB-DOT ORDERING: Advance 1 PPU dot BEFORE bus operation
        // ============================================================
        // This provides sub-dot ordering for VBlank boundary suppression:
        // - If dot 1 of scanline 241 occurs HERE, VBlank is set BEFORE the CPU read
        // - If dot 1 occurs in the remaining 2 dots (after CPU read), boundary suppression applies
        let ppu_frame_1 = self.ppu.borrow_mut().advance_dots(1)?;
        if let Some(frame) = ppu_frame_1 {
            out_frame = Some(frame);
        }

        // ============================================================
        // STEP 3: EXECUTE BUS OPERATION FOR THE SELECTED MASTER
        // ============================================================

        // Step DMA controller if active (handles state transitions including PendingHalt → Halt)
        // This must be called even during PendingHalt to advance the state machine.
        let dma_result = if self.dma_controller.is_active() {
            Some(self.dma_controller.step_cycle(cpu_is_writing, pending_read_addr, current_phase)
                .map_err(|e| NesConsoleError::InternalError(format!("DMA error: {}", e)))?)
        } else {
            None
        };

        // Handle DMC sample if DMA completed
        if let Some(ref result) = dma_result {
            if let Some(sample) = result.dmc_sample {
                self.apu.borrow_mut().provide_dmc_sample(sample)
                    .map_err(|e| NesConsoleError::InternalError(format!("APU error: {}", e)))?;
            }
        }

        // CPU executes if not stalled by DMA AND DMA didn't use the bus.
        // Important: PendingHalt does NOT stall - CPU continues until halt actually succeeds.
        //
        // INVARIANT: Exactly one bus operation per CPU cycle.
        // - If DMA returned a bus op, CPU must NOT execute (DMA owns bus)
        // - If DMA returned BusOp::None AND CPU not stalled, CPU executes
        // This protects the one-bus-op invariant even if DMA ever does something during PendingHalt.
        let dma_used_bus = dma_result
            .as_ref()
            .map(|r| !matches!(r.bus_op, crate::dma_controller::BusOp::None))
            .unwrap_or(false);

        let cpu_result = if self.dma_controller.is_cpu_stalled() || dma_used_bus {
            // CPU is stalled OR DMA used the bus - DMA is bus master
            let dma_result = dma_result.expect("DMA should have result when CPU is stalled or DMA used bus");

            // Derive read/write info from bus_op
            let (memory_read, memory_write, address) = match dma_result.bus_op {
                crate::dma_controller::BusOp::Read(addr) => (true, false, Some(addr)),
                crate::dma_controller::BusOp::Write(addr, _) => (false, true, Some(addr)),
                crate::dma_controller::BusOp::None => (false, false, None),
            };

            CpuCycleResult {
                halted: true,
                instruction_complete: false,
                memory_read,
                memory_write,
                address,
                ..Default::default()
            }
        } else {
            // CPU is bus master (DMA inactive, or DMA in PendingHalt with no bus op)
            let result = self.cpu.borrow_mut().step_cycle()?;

            // Track the CPU's last bus address for DMA repeated reads
            // This includes BOTH reads and writes - the real NES bus shows the
            // last accessed address during DMA stalls, regardless of access type.
            //
            // IMPORTANT: Only update if address is Some. If CPU reports memory_read=true
            // but address=None (should never happen, but defensive), we keep the previous
            // value rather than overwriting with None and losing our fallback.
            if (result.memory_read || result.memory_write) && result.address.is_some() {
                self.last_cpu_bus_address = result.address;
            }

            result
        };

        // ============================================================
        // Scheduler invariant assertion (catches double-stepping or starvation)
        // ============================================================
        // - If DMA used bus OR CPU stalled: CPU must NOT have stepped (halted=true)
        // - If DMA didn't use bus AND CPU not stalled: CPU must have stepped (halted=false)
        let cpu_should_be_halted = self.dma_controller.is_cpu_stalled() || dma_used_bus;
        debug_assert_eq!(
            cpu_result.halted, cpu_should_be_halted,
            "Scheduler invariant violated: cpu_result.halted={} but expected {} \
             (is_cpu_stalled={}, dma_used_bus={})",
            cpu_result.halted, cpu_should_be_halted,
            self.dma_controller.is_cpu_stalled(), dma_used_bus
        );

        // ============================================================
        // STEP 4: ADVANCE ALL COMPONENTS (AFTER BUS OPERATION)
        // ============================================================

        // Update cycle counters
        self.cpu_counter.current += 1;
        self.master_cycles += 1;

        // Note: Phase (GET/PUT) is derived from master_cycles + phase_offset at the start
        // of each cycle. No toggle needed - the increment above naturally advances parity.

        // Advance APU (AFTER the bus operation, not before)
        let (apu_cycles, apu_samples) = self.apu.borrow_mut().run(self.apu_counter.current, 1)?;
        if let Some(samples) = apu_samples {
            if let Some(existing) = out_samples.as_mut() {
                existing.append(samples);
            } else {
                out_samples = Some(samples);
            }
        }
        self.apu_counter.current = apu_cycles;

        // Advance PPU by remaining 2 dots (sub-dot ordering: 1 dot was advanced before bus op)
        // Total: 3 dots per CPU cycle for NTSC
        let ppu_frame_2 = self.ppu.borrow_mut().advance_dots(2)?;
        if let Some(frame) = ppu_frame_2 {
            out_frame = Some(frame);
        }
        self.ppu_counter.current += 1;

        // Clear per-cycle PPU state (VBlank boundary suppression latch)
        self.ppu.borrow_mut().end_master_cycle();

        // ============================================================
        // STEP 5: CHECK MAPPER IRQ (after PPU advances, since A12 edges happen during CHR reads)
        // ============================================================
        // MMC3 and similar mappers generate IRQ based on A12 rising edges during PPU
        // pattern table fetches. Poll the cartridge and signal/clear CPU IRQ accordingly.
        if let Some(ref cartridge) = self.cartridge {
            let mapper_irq_pending = cartridge.borrow().poll_irq();
            let cpu_has_mapper_irq = self.cpu.borrow().is_asserted_irq_by_source(MAPPER_IRQ)?;

            if mapper_irq_pending && !cpu_has_mapper_irq {
                // Mapper asserted IRQ - signal CPU
                self.cpu.borrow_mut().signal_irq(MAPPER_IRQ)?;
            } else if !mapper_irq_pending && cpu_has_mapper_irq {
                // Mapper cleared IRQ - clear CPU
                self.cpu.borrow_mut().clear_irq(MAPPER_IRQ)?;
            }
        }

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

    /// Execute a single CPU instruction using cycle-accurate stepping.
    /// Loops step_master_cycle() until instruction_complete is true.
    /// Returns an optional frame, optional samples, and a CPU snapshot for debugging.
    pub fn step_instruction_cycle_accurate(&mut self) -> Result<(Option<NesFrame>, Option<NesSamples>, Box<dyn CpuSnapshot>), NesConsoleError> {
        let mut out_frame: Option<NesFrame> = None;
        let mut out_samples: Option<NesSamples> = None;

        loop {
            let (cpu_result, frame, samples) = self.step_master_cycle()?;

            // Accumulate samples
            if let Some(s) = samples {
                if let Some(existing) = out_samples.as_mut() {
                    existing.append(s);
                } else {
                    out_samples = Some(s);
                }
            }

            // Capture frame if one completed
            if frame.is_some() {
                out_frame = frame;
            }

            // Stop when instruction completes (not during DMA halts)
            if cpu_result.instruction_complete {
                break;
            }
        }

        let snapshot = self.cpu.borrow().snapshot()?;
        Ok((out_frame, out_samples, snapshot))
    }

    /// Execute cycles until a complete frame is rendered, collecting CPU snapshots (cycle-accurate debug mode).
    /// Uses step_instruction_cycle_accurate() for per-instruction stepping.
    pub fn step_frame_debug_cycle_accurate(&mut self) -> Result<(NesFrame, NesSamples, Vec<Box<dyn CpuSnapshot>>), NesConsoleError> {
        let mut out_samples = NesSamples::default();
        let mut snapshots: Vec<Box<dyn CpuSnapshot>> = Vec::new();

        loop {
            let (frame, samples, snapshot) = self.step_instruction_cycle_accurate()?;
            snapshots.push(snapshot);

            if let Some(s) = samples {
                out_samples.append(s);
            }

            if let Some(frame) = frame {
                return Ok((frame, out_samples, snapshots));
            }
        }
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

        // Randomize phase offset on power-on (not on reset)
        // The real NES has indeterminate initial phase
        // Pass None for non-deterministic (realism mode)
        self.randomize_phase_offset(None);

        Ok(())
    }

    /// Power on with deterministic phase offset for testing.
    ///
    /// # Arguments
    /// * `phase_seed` - Seed for phase selection (seed % 2 == 0 → offset 0, else offset 1)
    pub fn power_on_deterministic(&mut self, phase_seed: u64) -> Result<(), NesConsoleError> {
        self.reset_entry_point()?;
        self.randomize_phase_offset(Some(phase_seed));
        Ok(())
    }

    /// Randomize the phase offset for GET/PUT cycle parity.
    ///
    /// On real hardware, the initial APU phase alignment is indeterminate.
    /// This randomization ensures games don't rely on a specific initial state.
    ///
    /// The phase offset is 0 or 1, added to master_cycles when computing phase:
    /// - offset 0: cycle 0 = GET, cycle 1 = PUT, cycle 2 = GET, ...
    /// - offset 1: cycle 0 = PUT, cycle 1 = GET, cycle 2 = PUT, ...
    ///
    /// # Arguments
    /// * `seed` - If `Some(n)`, uses deterministic selection (n % 2).
    ///            If `None`, uses wall-clock time (non-deterministic).
    fn randomize_phase_offset(&mut self, seed: Option<u64>) {
        self.phase_offset = match seed {
            Some(s) => s % 2,
            None => std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| (d.as_nanos() % 2) as u64)
                .unwrap_or(0),
        };
    }

    fn reset_counters(&mut self) {
        self.cpu_counter = CyclesCounter::new(CYCLE_START_SEQUENCE);
        self.apu_counter = CyclesCounter::new(0);
        // PPU counter starts at CYCLE_START_SEQUENCE to match CPU counter.
        // This ensures both instruction-level and cycle-accurate modes are synchronized.
        self.ppu_counter = CyclesCounter::new(CYCLE_START_SEQUENCE);
        self.dma_start_page.set(None);
        // Note: phase_offset is NOT reset here - it persists across reset.
        // Only randomized on power_on(). The DmaController.reset() also preserves phase.
        // Since phase is derived from (master_cycles + phase_offset), resetting
        // master_cycles to 0 restores the same starting phase as power-on.
        self.master_cycles = 0;
        self.dma_controller.reset();
        self.last_cpu_bus_address = None;
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

// ============================================================================
// Test-only methods for NesConsole
// ============================================================================

#[cfg(test)]
impl NesConsole {
    /// Test helper: Get current PPU scanline (0-261 for NTSC)
    pub fn get_ppu_scanline(&self) -> u16 {
        self.ppu.borrow().get_scanline()
    }

    /// Test helper: Get current PPU dot (0-340)
    pub fn get_ppu_dot(&self) -> u16 {
        self.ppu.borrow().get_dot()
    }

    /// Test helper: Check if VBlank flag is set (without side effects)
    ///
    /// Unlike reading $2002 which clears VBlank, this peeks at the flag directly.
    pub fn is_vblank_set(&self) -> bool {
        use crate::ppu_2c02::Ppu2c02;
        // Downcast to Ppu2c02 to access test helper
        // This is safe in tests since we always use Ppu2c02
        let ppu = self.ppu.borrow();
        if let Some(ppu_2c02) = ppu.as_any().downcast_ref::<Ppu2c02>() {
            ppu_2c02.is_vblank_set()
        } else {
            false
        }
    }

    /// Test helper: Get master cycle count
    pub fn get_master_cycles(&self) -> u64 {
        self.master_cycles
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

    fn build_ppu_dma(&self, ppu_dma_type: &PpuDmaType, data_bus: Rc<Cell<u8>>) -> Result<Rc<RefCell<dyn BusDevice>>, NesConsoleError>{
        debug!("creating ppu dma {:?}", ppu_dma_type);

        let ppu_dma = match ppu_dma_type {
            PpuDmaType::NESPPUDMA => {
                // PpuDma now just signals OAM DMA start via shared cell,
                // actual transfer is handled by DmaController in the scheduler
                PpuDma::new_with_dma_signal(self.dma_start_page.clone(), data_bus)
            },
        };

        Ok(Rc::new(RefCell::new(ppu_dma)))
    }

    fn build_ppu_device(&mut self, ppu_type: &PpuType, chr_rom: Rc<RefCell<dyn BusDevice>>,
                        mirroring: Rc<RefCell<PpuNameTableMirroring>>, bus: Rc<RefCell<dyn Bus>>,
                        cpu: Rc<RefCell<dyn CPU>>, config: ConfigSpec, data_bus: Rc<Cell<u8>>) -> Result<(Rc<RefCell<dyn BusDevice>>, Rc<RefCell<dyn BusDevice>>), NesConsoleError> {
        debug!("creating ppu {:?}", ppu_type);

        let result = match ppu_type {
            PpuType::NES2C02 => {
                Ppu2c02::new(chr_rom, mirroring, cpu, config)?
            },
        };

        let ppu = Rc::new(RefCell::new(result));
        let dma = self.build_ppu_dma(&PpuDmaType::NESPPUDMA, data_bus)?;

        ppu.borrow_mut().initialize()?;
        dma.borrow_mut().initialize()?;

        self.ppu = Some(ppu.clone());
        self.ppu_type = Some(ppu_type.clone());

        // Set cartridge reference on PPU for mapper A12 notifications (MMC3 scanline counter)
        if let Some(ref cartridge) = self.cartridge {
            ppu.borrow_mut().set_cartridge(cartridge.clone());
        }

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

    fn build_apu_device(&mut self, apu_type: &ApuType, cpu: Rc<RefCell<dyn CPU>>, config: ConfigSpec, data_bus: Rc<Cell<u8>>) -> Result<Rc<RefCell<dyn BusDevice>>, NesConsoleError> {
        debug!("creating apu {:?}", apu_type);

        let result = match apu_type {
            ApuType::RP2A03 => {
                let sound_player = SoundPlaybackPassive::new();
                ApuRp2A03::new(sound_player, cpu, config, data_bus)
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

                let (ppu, dma) = self.build_ppu_device(ppu_type, chr_rom, mirroring, bus.clone(), cpu, self.config.clone(), data_bus.clone())?;

                bus.borrow_mut().add_device(ppu)?;
                bus.borrow_mut().add_device(dma)?;
            },

            BusDeviceType::CONTROLLER(controller_type) => {
                let controller = self.build_controller_device(controller_type, data_bus.clone())?;
                bus.borrow_mut().add_device(controller.clone())?;
                self.controller = Some(controller.clone());
            }

            BusDeviceType::APU(apu_type) => {
                let apu = self.build_apu_device(apu_type, cpu, self.config.clone(), data_bus)?;
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

        // Create DmaController with bus (OAM writes go through bus to $2004)
        let dma_controller = DmaController::new(bus);

        let console = NesConsole::new(
            cpu,
            ppu,
            apu,
            controller,
            self.entry_point.take(),
            self.config,
            self.dma_start_page,
            dma_controller,
            self.cartridge.take(),
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