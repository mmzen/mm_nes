use std::cell::{Cell, RefCell};
use std::fmt::Debug;
use std::rc::Rc;
use log::{debug, info, trace};
use crate::apu::{ApuError, APU};
use crate::apu::ApuType::RP2A03;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cpu::{CpuError, CPU};
use crate::memory::{Memory, MemoryError};
use crate::sound_playback::SoundPlayback;

const APU_NAME: &str = "APU RP2A03";
const APU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x4000, 0x4017);
const APU_EXTERNAL_MEMORY_SIZE: usize = 32;
const APU_RATE: f64 = 894_886.5;
const AUDIO_RATE: f64 = 44_100.0;
const APU_CYCLES_PER_SAMPLE: f64 = APU_RATE / AUDIO_RATE; // ~20.29

const DUTY_CYCLES: [[u8; 8]; 4] = [
    [0, 0, 0, 0, 0, 0, 0, 1],
    [0, 0, 0, 0, 0, 0, 1, 1],
    [0, 0, 0, 0, 1, 1, 1, 1],
    [1, 1, 1, 1, 1, 1, 0, 0],
];

const DUTY_CYCLES_NAMES: [&str; 4] = [
    "12.5%", "25%", "50%", "75%"
];

trait Tick {
    fn tick(&mut self);
}

#[derive(Debug)]
enum ChannelType {
    Pulse1,
    Pulse2,
    Triangle,
    Noise
}

trait Channel {
    #[allow(dead_code)]
    fn reset(&mut self);
    fn is_muted(&self) -> bool;
    fn get_sample(&self) -> f32;
}

#[derive(Debug)]
struct Sweep {
    enabled: bool,
    initial_divider: u8,
    shift: u8,
    divider: u8,
    negate: bool,
    reload: bool,
    target_period: u16,
    update_real_period: bool
}

impl Sweep {
    fn new() -> Self {
        Sweep {
            enabled: false,
            initial_divider: 0,
            shift: 0,
            divider: 0,
            negate: false,
            reload: false,
            target_period: 0,
            update_real_period: false
        }
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.enabled = false;
        self.initial_divider = 0;
        self.shift = 0;
        self.divider = 0;
        self.negate = false;
        self.reload = false;
        self.target_period = 0;
        self.update_real_period = false;
    }

    fn compute_target_period(&self, timer_period: u16) -> u16 {
        timer_period >> self.shift
    }
}

impl Tick for Sweep {
    fn tick(&mut self) {
        if self.reload == true {
            self.divider = self.initial_divider;
            self.reload = false;
        } else if self.divider == 0 {
            if self.shift > 0 && self.enabled {
                self.update_real_period = true;
            }

            self.divider = self.initial_divider;
        } else {
            self.divider -= 1;
        }
    }
}

#[derive(Debug)]
struct Envelope {
    start_flag: bool,
    loop_flag: bool,
    const_volume: bool,
    counter: u8,
    divider: u8,
    volume: u8,
}

impl Envelope {
    fn new() -> Self {
        Envelope {
            start_flag: false,
            loop_flag: false,
            const_volume: false,
            counter: 0,
            divider: 0,
            volume: 0,
        }
    }

    fn get_volume(&self) -> u8 {
        if self.const_volume == true {
            self.volume
        } else {
            self.counter
        }
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.start_flag = false;
        self.loop_flag = false;
        self.const_volume = false;
        self.counter = 0;
        self.divider = 0;
        self.volume = 0;
    }
}

impl Tick for Envelope {
    fn tick(&mut self) {
        if self.start_flag {
            self.start_flag = false;
            self.counter = 15;
            self.divider = self.volume;
        } else {
            if self.divider > 0 {
                self.divider -= 1;
            } else {
                self.divider = self.volume;

                if self.counter > 0 {
                    self.counter -= 1;
                } else if self.loop_flag {
                    self.counter = 15;
                }
            }
        }
    }
}

#[derive(Debug)]
struct LengthCounter {
    halt: bool,
    counter: u8,
    counter_initial: u8
}

impl LengthCounter {
    const LENGTH_COUNTER_LOOKUP_TABLE: [u8; 32] = [
        10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
        12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
    ];

    fn new() -> Self {
        LengthCounter {
            halt: false,
            counter: 0,
            counter_initial: 0
        }
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.halt = false;
        self.counter = 0;
        self.counter_initial = 0;
    }

    fn reload(&mut self) {
        self.counter = self.counter_initial;
    }
}

impl Tick for LengthCounter {
    fn tick(&mut self) {
        if self.halt == false && self.counter > 0 {
            self.counter -= 1;
        }
    }
}

#[derive(Debug)]
struct LinearCounter {
    period: u8,
    counter: u8,
    reload: bool,
    control: bool,
}

impl LinearCounter {
    fn new() -> Self {
        LinearCounter {
            period: 0,
            counter: 0,
            reload: false,
            control: false
        }
    }

    #[allow(dead_code)]
    fn reset(&mut self) {
        self.period = 0;
        self.counter = 0;
        self.reload = false;
        self.control = false;
    }
}

impl Tick for LinearCounter {
    fn tick(&mut self) {
        if self.reload {
            self.counter = self.period;
        } else if self.counter > 0 {
            self.counter -= 1;
        }

        if self.control == false {
            self.reload = false;
        }
    }
}

#[derive(Debug)]
struct Pulse {
    enabled: bool,
    duty_cycle: usize,
    duty_cycle_index: usize,
    timer_period: u16,
    timer_counter: u16,
    sweep: Sweep,
    envelope: Envelope,
    length_counter: LengthCounter
}

impl Channel for Pulse {
    fn reset(&mut self) {
        self.enabled = false;
        self.timer_period = 0;
        self.timer_counter = 0;
        self.duty_cycle = 0;
        self.duty_cycle_index = 0;
        self.sweep.reset();
        self.envelope.reset();
        self.length_counter.reset();
    }

    fn is_muted(&self) -> bool {
        if self.enabled == false ||
            self.length_counter.counter == 0 ||
            self.timer_period < 8 ||
            (self.sweep.enabled && self.sweep.target_period > 0x07FF) {
            return true;
        }

        false
    }

    fn get_sample(&self) -> f32 {
        if self.is_muted() == true || self.duty_bit() == 0 {
            0.0
        } else {
            self.envelope.get_volume() as f32
        }
    }
}

impl Pulse {

    fn new() -> Self {
        Pulse {
            enabled: false,
            timer_period: 0,
            timer_counter: 0,
            duty_cycle: 0,
            duty_cycle_index: 0,
            sweep: Sweep::new(),
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
        }
    }

    fn duty_name(&self) -> &str {
        DUTY_CYCLES_NAMES[self.duty_cycle]
    }

    fn duty_position(&self) -> &str {
        let position = DUTY_CYCLES[self.duty_cycle][self.duty_cycle_index];

        match position {
            0 => "DOWN",
            1 => "UP",
            _ => unreachable!()
        }
    }

    fn duty_bit(&self) -> u8 {
        DUTY_CYCLES[self.duty_cycle][self.duty_cycle_index]
    }
}

#[derive(Debug)]
enum ShiftMode {
    Zero,
    One
}
const NOISE_PERIOD_DURATIONS: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068
];

#[derive(Debug)]
struct Noise {
    enabled: bool,
    timer_period: u16,
    timer_counter: u16,
    shift_register: u16,
    shift_mode: ShiftMode,
    envelope: Envelope,
    length_counter: LengthCounter
}

impl Channel for Noise {
    fn reset(&mut self) {
        self.enabled = false;
        self.timer_period = 0;
        self.timer_counter = 0;
        self.shift_register = 1;
        self.shift_mode = ShiftMode::Zero;
        self.envelope.reset();
        self.length_counter.reset();
    }

    fn is_muted(&self) -> bool {
        self.enabled == false || self.length_counter.counter == 0
    }

    fn get_sample(&self) -> f32 {
        if self.is_muted() == true || self.shift_register & 0x01 == 0x01 {
            0.0
        } else {
            self.envelope.get_volume() as f32
        }
    }
}

impl Noise {
    fn new() -> Self {
        Noise {
            enabled: false,
            timer_period: 0,
            timer_counter: 0,
            shift_register: 1,
            shift_mode: ShiftMode::Zero,
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
        }
    }

    fn period(value: u8) -> u16 {
        NOISE_PERIOD_DURATIONS[value as usize] / 2
    }
}

const TRIANGLE_SEQUENCES: [f32; 32] = [
    15.0, 14.0, 13.0, 12.0, 11.0, 10.0,  9.0,  8.0,  7.0,  6.0,  5.0,  4.0,  3.0,  2.0,  1.0,  0.0,
    0.0,  1.0,  2.0,  3.0,  4.0,  5.0,  6.0,  7.0,  8.0,  9.0, 10.0, 11.0, 12.0, 13.0, 14.0, 15.0
];

#[derive(Debug)]
struct Triangle {
    enabled: bool,
    timer_period: u16,
    timer_counter: u16,
    linear_counter: LinearCounter,
    length_counter: LengthCounter,
    sequencer_step: usize,
}

impl Channel for Triangle {
    fn reset(&mut self) {
        self.enabled = false;
        self.timer_period = 0;
        self.timer_counter = 0;
        self.linear_counter.reset();
        self.length_counter.reset();
        self.sequencer_step = 0;
    }

    fn is_muted(&self) -> bool {
        if self.enabled == false ||
            self.length_counter.counter == 0 ||
            self.linear_counter.counter == 0 ||
            self.timer_period < 2 {
            return true;
        }

        false
    }

    /***
     * to check: https://forums.nesdev.org/viewtopic.php?t=10658
     ***/
    fn get_sample(&self) -> f32 {
        if self.is_muted() == true {
            0.0
        } else {
            self.sequence()
        }
    }
}

impl Triangle {

    fn new() -> Self {
        Triangle {
            enabled: false,
            timer_period: 0,
            timer_counter: 0,
            linear_counter: LinearCounter::new(),
            length_counter: LengthCounter::new(),
            sequencer_step: 0
        }
    }

    fn sequence(&self) -> f32 {
        TRIANGLE_SEQUENCES[self.sequencer_step]
    }

    fn num_of_sequences(&self) -> usize {
        TRIANGLE_SEQUENCES.len()
    }
}

const DMC_PERIODS: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54
];

#[derive(Debug)]
enum DmcBufferState {
    Empty,
    NotEmpty
}

#[derive(Debug)]
struct Dmc<U: CPU + ?Sized> {
    dmc_irq: bool,
    timer_period: u16,
    timer_counter: u16,
    loop_flag: bool,
    output_level: u8,
    sample_address: u16,
    current_address: u16,
    sample_length: u16,
    sample_buffer: u8,
    sample_state: DmcBufferState,
    bytes_remaining: u16,
    shift_register: u8,
    bits_remaining: u8,
    silenced: bool,
    cpu: Rc<RefCell<U>>
}

impl<U: CPU + ?Sized> Channel for Dmc<U> {
    fn reset(&mut self) {
        self.dmc_irq = false;
        self.timer_period = 0;
        self.timer_counter = 0;
        self.loop_flag = false;
        self.output_level = 0;
        self.sample_address = 0;
        self.current_address = 0;
        self.sample_length = 0;
        self.sample_buffer = 0;
        self.sample_state = DmcBufferState::Empty;
        self.bytes_remaining = 0;
        self.shift_register = 0;
        self.bits_remaining = 0;
        self.silenced = false;
    }

    fn is_muted(&self) -> bool {
        self.silenced == true
    }

    fn get_sample(&self) -> f32 {
        self.output_level as f32
    }
}

impl<U: CPU + ?Sized> Dmc<U> {
    fn new(cpu: Rc<RefCell<U>>) -> Self {
        Dmc {
            dmc_irq: false,
            timer_period: 0,
            timer_counter: 0,
            loop_flag: false,
            output_level: 0,
            sample_address: 0,
            current_address: 0,
            sample_length: 0,
            sample_buffer: 0,
            sample_state: DmcBufferState::Empty,
            bytes_remaining: 0,
            shift_register: 0,
            bits_remaining: 0,
            silenced: false,
            cpu
        }
    }

    fn period(value: u8) -> u16 {
        DMC_PERIODS[value as usize] / 2
    }

    fn restart(&mut self) {
        self.current_address = self.sample_address;
        self.bytes_remaining = self.sample_length;
    }

    fn interrupt(&mut self) -> Result<(), ApuError> {
        debug!("APU: signaling interrupt from dmc");
        self.cpu.borrow_mut().signal_irq()?;
        Ok(())
    }

    fn increment_sample_address(&mut self) {
        self.current_address = if self.current_address == 0xFFFF {
            0x8000
        } else {
            self.current_address + 1
        };
    }

    fn cycle_output(&mut self) {
        if self.timer_counter == 0 {
            if self.bits_remaining == 0 {

                if let DmcBufferState::Empty = self.sample_state {
                    self.silenced = true;
                } else {
                    self.silenced = false;
                    self.shift_register = self.sample_buffer;
                    self.sample_state = DmcBufferState::NotEmpty;
                    self.bits_remaining = 8;
                }
            } else {
                self.bits_remaining -= 1;
            }

            self.timer_counter = self.timer_period;
        } else {
            self.timer_counter -= 1;
        }
    }

    fn update_output_level(&mut self) {
        if self.silenced == false {
            let bit_0 = self.shift_register & 0x01;

            if bit_0 == 1 && self.output_level <= 125 {
                self.output_level += 2;
            } else if self.output_level >= 2 {
                self.output_level -= 2;
            }

            self.shift_register >>= 1;
        }
    }

    fn conditional_refill(&mut self) -> Result<(), ApuError> {
        if let DmcBufferState::Empty = self.sample_state {
            if self.bytes_remaining > 0 {
                self.sample_buffer = 60; // XXX load sample data
                self.increment_sample_address();
                self.bytes_remaining -= 1;

                if self.bytes_remaining == 0 {
                    if self.loop_flag == true {
                        self.restart()
                    }

                    if self.dmc_irq == true {
                        self.interrupt()?;
                    }
                } else {
                    self.sample_state = DmcBufferState::NotEmpty;
                }
            }
        }

        Ok(())
    }
}

#[derive(Debug)]
enum FrameCounterMode {
    FourStep,
    FiveStep
}

const FRAME_COUNTER_4_STEPS_EVENTS: [u32; 4] = [3728, 7456, 11185, 14914];

/***
 * quarter_frame, half_frame, interrupt flag
 */
const FRAME_COUNTER_4_STEPS_SEQUENCES: [(bool, bool, bool); 4] = [
    (true,  false, false), // step 0 (3728)  : quarter
    (true,  true, false ), // step 1 (7456)  : quarter + half
    (true,  false, false), // step 2 (11185) : quarter
    (true,  true, true ), // step 3 (14914) : quarter + half + irq
];

const FRAME_COUNTER_5_STEPS_EVENTS: [u32; 5] = [3728, 7456, 11185, 14914, 18640];

const FRAME_COUNTER_5_STEPS_SEQUENCES: [(bool, bool, bool); 5] = [
    (true,  false, false), // step 0 (3728)  : quarter
    (true,  true, false ), // step 1 (7456)  : quarter + half
    (true,  false, false), // step 2 (11185) : quarter
    (false, false, false), // step 3 (14914) : nothing
    (true,  true, false), // step 4 (18640) : quarter + half
];

#[derive(Debug)]
struct FrameCounter<U: CPU + ?Sized> {
    mode: FrameCounterMode,
    inhibit_irq: Cell<bool>,
    apu_cycle: u32,
    next_step: usize,
    cpu: Rc<RefCell<U>>
}

impl<U: CPU + ?Sized> FrameCounter<U> {
    fn new(cpu: Rc<RefCell<U>>) -> Self {
        FrameCounter {
            mode: FrameCounterMode::FourStep,
            inhibit_irq: Cell::new(false),
            apu_cycle: 0,
            next_step: 0,
            cpu
        }
    }

    fn reset(&mut self) {
        self.mode = FrameCounterMode::FourStep;
        self.inhibit_irq = Cell::new(false);
        self.apu_cycle = 0;
        self.next_step = 0;
    }

    fn frame_tables(&self) -> (&'static [u32], &'static [(bool, bool, bool)]) {
        match self.mode {
            FrameCounterMode::FourStep => (&FRAME_COUNTER_4_STEPS_EVENTS, &FRAME_COUNTER_4_STEPS_SEQUENCES),
            FrameCounterMode::FiveStep => (&FRAME_COUNTER_5_STEPS_EVENTS, &FRAME_COUNTER_5_STEPS_SEQUENCES),
        }
    }

    fn interrupt(&mut self) -> Result<(), ApuError> {
        debug!("APU: signaling interrupt from frame counter");
        self.cpu.borrow_mut().signal_irq()?;
        Ok(())
    }
}

#[derive(Debug)]
pub struct ApuRp2A03<T: SoundPlayback, U: CPU + ?Sized> {
    pulse1: Pulse,
    pulse2: Pulse,
    noise: Noise,
    triangle: Triangle,
    dmc: Dmc<U>,
    frame_counter: FrameCounter<U>,
    apu_cycles_acc: f64,
    sound_player: T
}

impl<T: SoundPlayback, U: CPU + ?Sized> BusDevice for ApuRp2A03<T, U> {
    fn get_name(&self) -> String {
        APU_NAME.to_string()
    }

    fn get_device_type(&self) -> BusDeviceType {
        BusDeviceType::APU(RP2A03)
    }

    fn get_address_range(&self) -> (u16, u16) {
        APU_EXTERNAL_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        APU_EXTERNAL_ADDRESS_SPACE.0 <= addr && addr <= APU_EXTERNAL_ADDRESS_SPACE.1
    }
}

impl<T: SoundPlayback, U: CPU + ?Sized> Memory for ApuRp2A03<T, U> {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing APU");
        Ok(APU_EXTERNAL_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        trace!("APU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);

        let value = match addr {
            0x00 | 0x01 | 0x02 | 0x03 |
            0x04 | 0x05 | 0x06 | 0x07 => self.read_pulse(addr)?,
            0x15 => self.read_channels_status()?,
            _ => {
                trace!("APU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);
                0
            },
        };

        Ok(value)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        self.read_byte(addr)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        trace!("APU: registers access: write byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);

        match addr {
            0x00 => self.write_pulse_control(ChannelType::Pulse1, value)?,
            0x01 => self.write_pulse_sweep(ChannelType::Pulse1, value)?,
            0x02 => self.write_pulse_timer_lo(ChannelType::Pulse1, value)?,
            0x03 => self.write_length_counter_and_timer_hi(ChannelType::Pulse1, value)?,
            0x04 => self.write_pulse_control(ChannelType::Pulse2, value)?,
            0x05 => self.write_pulse_sweep(ChannelType::Pulse2, value)?,
            0x06 => self.write_pulse_timer_lo(ChannelType::Pulse2, value)?,
            0x07 => self.write_length_counter_and_timer_hi(ChannelType::Pulse2, value)?,
            0x08 => self.write_triangle_linear_counter(value)?,
            0x10 => self.write_dmc_flag_and_rate(value)?,
            0x11 => self.write_dmc_output(value)?,
            0x12 => self.write_dmc_sample_address(value)?,
            0x13 => self.write_dmc_sample_length(value)?,
            0x15 => self.write_channels_status(value)?,
            0x17 => self.write_frame_counter(value)?,
            0x0A => self.write_pulse_timer_lo(ChannelType::Triangle, value)?,
            0x0B => self.write_length_counter_and_timer_hi(ChannelType::Triangle, value)?,
            0x0C => self.write_noise_control(value)?,
            0x0E => self.write_noise_mode_and_period(value)?,
            0x0F => self.write_length_counter_and_envelope_restart(ChannelType::Noise, value)?,
            _ => debug!("APU: registers access: write byte at 0x{:04X} (0x{:04X}): 0x{:02X}", addr, addr + 0x4000, value),
        };

        Ok(())
    }

    fn read_word(&self, _: u16) -> Result<u16, MemoryError> {
        Ok(0)
    }

    fn write_word(&mut self, _: u16, _: u16) -> Result<(), MemoryError> {
        Ok(())
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        APU_EXTERNAL_MEMORY_SIZE
    }
}

impl<T: SoundPlayback, U: CPU + ?Sized> ApuRp2A03<T, U> {
    pub fn new(sound_player: T, cpu: Rc<RefCell<U>>) -> Self {
        ApuRp2A03 {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            noise: Noise::new(),
            triangle: Triangle::new(),
            dmc: Dmc::new(cpu.clone()),
            frame_counter: FrameCounter::new(cpu.clone()),
            sound_player,
            apu_cycles_acc: 0.0,
        }
    }

    fn read_pulse(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(0)
    }

    fn get_pulse_channel_by_type(&mut self, channel_type: &ChannelType) -> &mut Pulse {
        match channel_type {
            ChannelType::Pulse1 => &mut self.pulse1,
            ChannelType::Pulse2 => &mut self.pulse2,
            _ => { unreachable!() }
        }
    }

    fn write_pulse_control(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.duty_cycle = ((value & 0xC0) >> 6) as usize;
        pulse.length_counter.halt = (value & 0x20) != 0;
        pulse.envelope.loop_flag = (value & 0x20) != 0;
        pulse.envelope.const_volume = (value & 0x10) != 0;
        pulse.envelope.divider = value & 0x0F;
        pulse.envelope.volume = value & 0x0F;

        trace!("APU: updated pulse control (0x{:02X}) {:?}: enabled: {}, duty: {} ({}), length counter halt: {}, loop: {}, constant volume: {}, divider: {}, volume: {}",
                 value, channel_type, pulse.enabled, pulse.duty_name(), pulse.duty_position(),
                 pulse.length_counter.halt, pulse.envelope.loop_flag, pulse.envelope.const_volume, pulse.envelope.divider, pulse.envelope.volume);

        Ok(())
    }

    fn write_pulse_sweep(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.sweep.enabled = (value & 0x80) != 0;
        pulse.sweep.initial_divider = ((value & 0x70) >> 4) + 1;
        pulse.sweep.negate = (value & 0x08) != 0;
        pulse.sweep.shift = value & 0x07;
        pulse.sweep.reload = true;

        trace!("APU: updated pulse sweep: enabled: {}, divider: {}, negate: {}, shift: {}, reload: {}",
             pulse.sweep.enabled, pulse.sweep.initial_divider, pulse.sweep.negate, pulse.sweep.shift, pulse.sweep.reload);

        Ok(())
    }

    /***
     * 0x4002 and 0x4006 - pulse timer (period) low 8 bits
     * 0x400A - triangle timer (period) low 8 bits
     *
     * https://www.nesdev.org/wiki/APU_Pulse
     ***/
    fn write_pulse_timer_lo(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {

        match channel_type {
            ChannelType::Pulse1 |
            ChannelType::Pulse2 => {
                let pulse = self.get_pulse_channel_by_type(&channel_type);

                pulse.timer_period = (pulse.timer_period & 0xFF00) | value as u16;
                trace!("APU: updated pulse timer low byte: 0x{:04X}", pulse.timer_period);
            },
            ChannelType::Triangle => {
                let triangle = &mut self.triangle;

                triangle.timer_period = (triangle.timer_period & 0xFF00) | value as u16;
                trace!("APU: updated triangle timer low byte: 0x{:04X}", triangle.timer_period);
            },
            _ => { unreachable!() }
        }

        Ok(())
    }

    /***
     * 0x4003 and 0x4007 - pulse length counter load and timer (period) high 3 bits
     * 0x400B - triangle length counter load and timer (period) high 3 bits
     *
     * https://www.nesdev.org/wiki/APU_Pulse
     ***/
    fn write_length_counter_and_timer_hi(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {

        match channel_type {
            ChannelType::Pulse1 |
            ChannelType::Pulse2 => {
                let pulse = self.get_pulse_channel_by_type(&channel_type);

                pulse.timer_period = (pulse.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
                pulse.timer_counter = pulse.timer_period;

                if pulse.enabled {
                    pulse.length_counter.counter_initial = LengthCounter::LENGTH_COUNTER_LOOKUP_TABLE[(value >> 3) as usize];
                    pulse.length_counter.reload();
                }

                pulse.envelope.start_flag = true;
                pulse.duty_cycle_index = 0;
            },
            ChannelType::Triangle => {
                let triangle = &mut self.triangle;

                triangle.timer_period = (triangle.timer_period & 0x00FF) | (((value & 0x07) as u16) << 8);
                triangle.timer_counter = triangle.timer_period;

                if triangle.enabled {
                    triangle.length_counter.counter_initial = LengthCounter::LENGTH_COUNTER_LOOKUP_TABLE[(value >> 3) as usize];
                    triangle.length_counter.reload();
                }

                triangle.linear_counter.reload = true;
            }
            _ => { unreachable!() }
        }

        Ok(())
    }

    /***
     * 0x4008 - triangle linear counter
     */
    fn write_triangle_linear_counter(&mut self, value: u8) -> Result<(), MemoryError> {
        let triangle = &mut self.triangle;

        triangle.linear_counter.period = value & 0x7F;
        triangle.linear_counter.control = (value & 0x80) == 0x80;
        self.triangle.length_counter.halt = (value & 0x80) == 0x80;

        Ok(())
    }

    /***
     * 0x4010
     */
    fn write_dmc_flag_and_rate(&mut self, value: u8) -> Result<(), MemoryError> {
        let dmc = &mut self.dmc;

        dmc.dmc_irq = (value & 0x80) != 0;
        dmc.loop_flag = (value & 0x40) != 0;
        dmc.timer_period = Dmc::<U>::period(value & 0x0F);

        Ok(())
    }

    /***
     * 0x4011
     */
    fn write_dmc_output(&mut self, value: u8) -> Result<(), MemoryError> {
        let dmc = &mut self.dmc;
        dmc.output_level = value & 0x7F;

        Ok(())
    }

    /***
     * 0x4012
     */
    fn write_dmc_sample_address(&mut self, value: u8) -> Result<(), MemoryError> {
        self.dmc.sample_address = 0xC000 | ((value as u16)<< 6);
        Ok(())
    }

    /***
     * 0x4013
     */
    fn write_dmc_sample_length(&mut self, value: u8) -> Result<(), MemoryError> {
        self.dmc.sample_length = ((value as u16) << 4) | 0x01;
        Ok(())
    }

    /***
     * 0x4015
     *
     * https://www.nesdev.org/wiki/APU#Status_($4015)
     ***/
    fn read_channels_status(&self) -> Result<u8, MemoryError> {
        let pulse1 = self.pulse1.length_counter.counter > 0;
        let pulse2 = self.pulse2.length_counter.counter > 0;
        let triangle = self.triangle.length_counter.counter > 0;
        let noise = self.noise.length_counter.counter > 0;
        let dmc = self.dmc.bytes_remaining > 0;
        let dmc_irq = self.dmc.dmc_irq;

        let mut status = (pulse1 as u8) | ((pulse2 as u8) << 1) | ((triangle as u8) << 2) | ((noise as u8) << 3) | ((dmc as u8) << 4);

        status |= (self.frame_counter.inhibit_irq.get() as u8) << 6;
        status |= (dmc_irq as u8) << 7;

        self.frame_counter.inhibit_irq.set(true);

        trace!("APU: channels status: pulse1: {}, pulse2: {}, triangle: {}, noise {}, status: 0x{:02X}",
             pulse1, pulse2, triangle, noise, status);

        Ok(status)
    }

    /***
     * 0x4015 - status
     ***/
    fn write_channels_status(&mut self, value: u8) -> Result<(), MemoryError> {
        self.pulse1.enabled = value & 0x01 != 0;
        self.pulse2.enabled = value & 0x02 != 0;
        self.triangle.enabled = value & 0x04 != 0;
        self.noise.enabled = value & 0x08 != 0;
        let dmc_bit = value & 0x10 != 0;

        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            if pulse.enabled == false {
                pulse.length_counter.counter = 0
            }
        }

        if self.triangle.enabled == false {
            self.triangle.length_counter.counter = 0;
        }

        if self.noise.enabled == false {
            self.noise.length_counter.counter = 0;
        }

        if dmc_bit == true && self.dmc.bytes_remaining == 0  {
            self.dmc.restart();
        } else {
            self.dmc.silenced = true;
            self.dmc.bytes_remaining = 0;
        }

        self.dmc.dmc_irq = false;

        trace!("APU: updated channels status: pulse1 enabled: {}, pulse2 enabled: {}, triangle: {}, noise enabled: {}",
             self.pulse1.enabled, self.pulse2.enabled, self.triangle.enabled, self.noise.enabled);

        Ok(())
    }

    fn write_frame_counter(&mut self, value: u8) -> Result<(), MemoryError> {
        self.frame_counter.mode = match (value & 0x80) != 0 {
            true => FrameCounterMode::FiveStep,
            false => FrameCounterMode::FourStep,
        };

        self.frame_counter.inhibit_irq.set((value & 0x40) != 0);

        self.frame_counter.next_step = 0;
        self.frame_counter.apu_cycle = 0;

        if let FrameCounterMode::FiveStep = self.frame_counter.mode {
            self.tick_envelopes();
            self.tick_length_counters();
            self.tick_sweep_units();
        }

        trace!("APU: updated frame counter: mode: {:?}, inhibit_irq: {}",
             self.frame_counter.mode, self.frame_counter.inhibit_irq.get());

        Ok(())
    }

    fn write_noise_control(&mut self, value: u8) -> Result<(), MemoryError> {
        let noise = &mut self.noise;

        noise.length_counter.halt = (value & 0x20) != 0;
        noise.envelope.loop_flag = (value & 0x20) != 0;
        noise.envelope.const_volume = (value & 0x10) != 0;
        noise.envelope.divider = value & 0x0F;
        noise.envelope.volume = value & 0x0F;

        trace!("APU: updated noise control (0x{:02X}): enabled: {}, length counter halt: {}, loop: {}, constant volume: {}, divider: {}, volume: {}",
                 value, noise.enabled, noise.length_counter.halt, noise.envelope.loop_flag,
                 noise.envelope.const_volume, noise.envelope.divider, noise.envelope.volume);

        Ok(())
    }

    fn write_noise_mode_and_period(&mut self, value: u8) -> Result<(), MemoryError> {
        let noise = &mut self.noise;

        let idx = value & 0x0F;
        noise.timer_period = Noise::period(idx);
        noise.shift_mode = if (value & 0x80) == 0 {
            ShiftMode::Zero
        } else {
            ShiftMode::One
        };

        Ok(())
    }

    fn write_length_counter_and_envelope_restart(&mut self, _: ChannelType, value: u8) -> Result<(), MemoryError> {
        let noise = &mut self.noise;

        noise.length_counter.counter_initial = LengthCounter::LENGTH_COUNTER_LOOKUP_TABLE[(value >> 3) as usize];
        noise.length_counter.reload();

        noise.envelope.start_flag = true;

        trace!("APU: updated length counter and envelope restart for noise channel: counter initial: {}, reload: {}",
                 noise.length_counter.counter_initial, noise.envelope.start_flag);

        Ok(())
    }

    fn clock_pulse_timers(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            if pulse.enabled == false {
                continue
            }

            if pulse.timer_counter == 0 {
                pulse.duty_cycle_index = (pulse.duty_cycle_index + 1) % 8;
                pulse.timer_counter = pulse.timer_period;
            } else {
                pulse.timer_counter -= 1;
            }
        }
    }

    fn clock_triangle_timer(&mut self) {
        let triangle = &mut self.triangle;

        if triangle.length_counter.counter > 0 && triangle.linear_counter.counter > 0 && triangle.timer_period >= 2 {
            if triangle.timer_counter == 0 {
                triangle.sequencer_step = (triangle.sequencer_step + 1) % triangle.num_of_sequences();
                triangle.timer_counter = triangle.timer_period;
            } else {
                triangle.timer_counter -= 1;
            }
        } else if triangle.timer_counter > 0 {
            triangle.timer_counter -= 1;
        }
    }

    fn clock_noise_timer(&mut self) {
        if self.noise.timer_counter == 0 {
            let shift_by = match self.noise.shift_mode {
                ShiftMode::Zero => { 1 },
                ShiftMode::One => { 6 }
            };

            let feedback = (self.noise.shift_register & 0x01) ^ ((self.noise.shift_register >> shift_by) & 0x01);
            self.noise.shift_register >>= 1;
            self.noise.shift_register |= feedback << 14;

            self.noise.timer_counter = self.noise.timer_period;
        } else {
            self.noise.timer_counter -= 1;
        }
    }

    fn clock_dmc_timer(&mut self) {
        let dmc = &mut self.dmc;

        dmc.cycle_output();
        dmc.update_output_level();
    }

    fn convert_cpu_cycles_to_apu_cycles(cpu_cycle: u32) -> u32 {
        cpu_cycle / 2
    }


    fn tick_sweep_units(&mut self) {

        for (idx, pulse) in [&mut self.pulse1, &mut self.pulse2].iter_mut().enumerate() {
            let is_pulse1 = idx == 0;

            pulse.sweep.tick();

            if pulse.sweep.update_real_period {
                if pulse.timer_period >= 8 && pulse.sweep.shift > 0 {
                    let mut delta= pulse.sweep.compute_target_period(pulse.timer_period);

                    let new_period = if pulse.sweep.negate {
                        if is_pulse1 == true {
                            delta = delta + 1;
                        }
                        pulse.timer_period.wrapping_sub(delta)
                    } else {
                        pulse.timer_period.wrapping_add(delta)
                    };

                    pulse.sweep.target_period = new_period;

                    if new_period <= 0x07FF {
                        pulse.timer_period = new_period & 0x07FF;
                    }
                }

                pulse.sweep.update_real_period = false;
            }
        }
    }

    fn tick_length_counters(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            pulse.length_counter.tick();
        }

        self.triangle.length_counter.tick();
        self.noise.length_counter.tick();
    }

    fn tick_envelopes(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            pulse.envelope.tick();
        }

        self.noise.envelope.tick();
    }

    fn tick_linear_counter(&mut self) {
        self.triangle.linear_counter.tick();
    }

    fn clock_frame_sequencer(&mut self, cycle: u32) -> Result<(), ApuError> {
        let (events, quarter_half_interrupt) = self.frame_counter.frame_tables();

        self.frame_counter.apu_cycle += cycle;

        for _ in events.iter() {
            let idx = self.frame_counter.next_step;

           if self.frame_counter.apu_cycle >= events[idx] {
               let do_quarter = quarter_half_interrupt[idx].0;
               let do_half = quarter_half_interrupt[idx].1;
               let do_interrupt = quarter_half_interrupt[idx].2;

               if do_quarter {
                   self.tick_envelopes();
                   self.tick_linear_counter();
               }

               if do_half {
                   self.tick_length_counters();
                   self.tick_sweep_units();
               }

               if do_interrupt && self.frame_counter.inhibit_irq.get() == false {
                   self.frame_counter.interrupt()?;
               }

               self.frame_counter.next_step += 1;

               if self.frame_counter.next_step >= events.len() {
                   self.frame_counter.next_step = 0;
                   self.frame_counter.apu_cycle = 0;
               }
           } else {
               break;
           }
        }

        Ok(())
    }

    fn pulse_out(&mut self) -> f32 {
        let sample1 = self.pulse1.get_sample();
        let sample2 = self.pulse2.get_sample();

        let sample_sum = sample1 + sample2;
        let sample = if sample_sum == 0.0 { 0.0 } else { (95.88) / ((8128.0 / (sample_sum)) + 100.0) };
        let out = (sample.min(1.0).max(0.0) * 2.0) - 1.0;

        out
    }

    fn tnd_out(&mut self) -> f32 {
        let noise = self.noise.get_sample();
        let triangle = self.triangle.get_sample();
        let dmc = self.dmc.get_sample();

        let tnd = triangle / 8227.0 + noise / 12241.0 + dmc / 22638.0;
        let tnd_out = if tnd == 0.0 { 0.0 } else { 159.79 / (1.0 / tnd + 100.0) };

        tnd_out
    }

    fn clock_mixer(&mut self) {
        let pulse_out = self.pulse_out();
        let tnd_out = self.tnd_out();

        self.sound_player.push_sample(pulse_out + tnd_out);
    }
}

impl<T: SoundPlayback, U: CPU + ?Sized> APU for ApuRp2A03<T, U> {
    fn reset(&mut self) -> Result<(), ApuError> {
        info!("resetting APU");
        self.pulse1.reset();
        self.pulse2.reset();
        self.triangle.reset();
        self.noise.reset();
        self.dmc.reset();
        self.frame_counter.reset();
        Ok(())
    }

    fn panic(&self, _: &ApuError) {
        unreachable!()
    }

    /***
     * General logic:
     *   - clock channels - every 1 APU cycle
     *   - clock the frame sequencer - every 1 APU cycle
     *   - perform sampling and mixing -
     *   - apply filters
     *   - send to audio device
     *
     * https://forums.nesdev.org/viewtopic.php?t=8602
     *
     ***/
    fn run(&mut self, _: u32, credits: u32) -> Result<u32, ApuError> {
        let apu_credits = ApuRp2A03::<T, U>::convert_cpu_cycles_to_apu_cycles(credits);

        for _ in 0..apu_credits {
            self.clock_pulse_timers();
            self.clock_triangle_timer();
            self.clock_noise_timer();
            self.clock_dmc_timer(); // should be clocked every CPU cycle and not APU cycle
            self.clock_frame_sequencer(1)?;

            self.apu_cycles_acc += 1.0;

            while self.apu_cycles_acc >= APU_CYCLES_PER_SAMPLE {
                self.clock_mixer();
                self.apu_cycles_acc -= APU_CYCLES_PER_SAMPLE;
            }
        }

        Ok(credits)
    }
}