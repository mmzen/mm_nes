use log::{debug, info, trace};
use crate::apu::{ApuError, APU};
use crate::apu::ApuType::RP2A03;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::sound_playback::SoundPlayback;

const APU_NAME: &str = "APU RP2A03";
const APU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x4000, 0x4017);
const APU_EXTERNAL_MEMORY_SIZE: usize = 32;
const MAX_SAMPLING_BUFFER_SIZE: usize = 44_100;
const MIXER_SAMPLING_CYCLE_THRESHOLD: u32 = 20;

#[derive(Debug)]
enum ChannelType {
    Pulse1,
    Pulse2
}

trait Channel {
    fn reset(&mut self);
    fn get_sample(&self) -> f32;
}

#[derive(Debug)]
struct Sweep {
    enabled: bool,
    period: u8,
    shift: u8,
    divider: u8,
    negate: bool,
    reload: bool
}

impl Sweep {
    fn new() -> Self {
        Sweep {
            enabled: false,
            period: 0,
            shift: 0,
            divider: 0,
            negate: false,
            reload: false
        }
    }

    fn reset(&mut self) {
        self.enabled = false;
        self.period = 0;
        self.shift = 0;
        self.divider = 0;
        self.negate = false;
        self.reload = false;
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
        if self.const_volume {
            self.volume
        } else {
            self.counter
        }
    }

    fn reset(&mut self) {
        self.start_flag = false;
        self.loop_flag = false;
        self.const_volume = false;
        self.counter = 0;
        self.divider = 0;
        self.volume = 0;
    }
}

#[derive(Debug)]
struct LengthCounter {
    enabled: bool,
    halt: bool,
    counter: u8,
    reload: u8
}

impl LengthCounter {
    const LENGTH_COUNTER_LOOKUP_TABLE: [u8; 32] = [
        10, 254, 20, 2, 40, 4, 80, 6, 160, 8, 60, 10, 14, 12, 26, 14,
        12, 16, 24, 18, 48, 20, 96, 22, 192, 24, 72, 26, 16, 28, 32, 30
    ];

    fn new() -> Self {
        LengthCounter {
            enabled: false,
            halt: false,
            counter: 0,
            reload: 0
        }
    }

    fn reset(&mut self) {
        self.halt = false;
        self.counter = 0;
        self.reload = 0;
    }
}

#[derive(Debug)]
struct Pulse {
    enabled: bool,
    period: u16,
    period_counter: u16,
    duty_cycle: usize,
    duty_cycle_index: usize,
    sweep: Sweep,
    envelope: Envelope,
    length_counter: LengthCounter,
}

impl Channel for Pulse {
    fn reset(&mut self) {
        self.enabled = false;
        self.period = 0;
        self.period_counter = 0;
        self.duty_cycle = 0;
        self.duty_cycle_index = 0;
        self.sweep.reset();
        self.envelope.reset();
        self.length_counter.reset();
    }

    fn get_sample(&self) -> f32 {
        (Self::DUTY_CYCLES[self.duty_cycle][self.duty_cycle_index] * self.envelope.get_volume()) as f32
    }
}

impl Pulse {
    const DUTY_CYCLES: [[u8; 8]; 4] = [
        [0, 0, 0, 0, 0, 0, 0, 1],
        [0, 0, 0, 0, 0, 0, 1, 1],
        [0, 0, 0, 0, 1, 1, 1, 1],
        [1, 1, 1, 1, 1, 1, 0, 0],
    ];

    fn new() -> Self {
        Pulse {
            enabled: false,
            period: 0,
            period_counter: 0,
            duty_cycle: 0,
            duty_cycle_index: 0,
            sweep: Sweep::new(),
            envelope: Envelope::new(),
            length_counter: LengthCounter::new(),
        }
    }
}

#[derive(Debug)]
enum FrameCounterMode {
    FourStep,
    FiveStep
}

#[derive(Debug)]
struct FrameCounter {
    mode: FrameCounterMode,
    inhibit_irq: bool,
    apu_cycle: u32
}

impl FrameCounter {
    fn get_frame_sequencer_step_by_cycle(cycle: u32) -> u8 {
        match cycle {
            3729 => 1,
            7457 => 2,
            11485 => 3,
            14914 => 4,
            18641 => 5,
            _ => 0,
        }
    }
}

impl FrameCounter {
    fn new() -> Self {
        FrameCounter {
            mode: FrameCounterMode::FourStep,
            inhibit_irq: false,
            apu_cycle: 0
        }
    }
}

#[derive(Debug)]
pub struct ApuRp2A03<T: SoundPlayback> {
    pulse1: Pulse,
    pulse2: Pulse,
    frame_counter: FrameCounter,
    last_mixer_cycle: u32,
    samples: [f32; MAX_SAMPLING_BUFFER_SIZE],
    samples_index: usize,
    sound_player: T
}

impl<T: SoundPlayback> BusDevice for ApuRp2A03<T> {
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

impl<T: SoundPlayback> Memory for ApuRp2A03<T> {
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
                debug!("APU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x4000);
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
            0x15 => self.write_channels_status(value)?,
            0x17 => self.write_frame_counter(value)?,
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

impl<T: SoundPlayback> ApuRp2A03<T> {
    pub fn new(sound_player: T) -> Self {
        ApuRp2A03 {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            frame_counter: FrameCounter::new(),
            last_mixer_cycle: 0,
            samples: [0.0; MAX_SAMPLING_BUFFER_SIZE],
            samples_index: 0,
            sound_player,
        }
    }

    fn read_pulse(&self, _: u16) -> Result<u8, MemoryError> {
        Ok(0)
    }

    fn get_pulse_channel_by_type(&mut self, channel_type: &ChannelType) -> &mut Pulse {
        match channel_type {
            ChannelType::Pulse1 => &mut self.pulse1,
            ChannelType::Pulse2 => &mut self.pulse2,
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

        trace!("APU: updated pulse control: duty: {} ({:?}), length counter halt: {}, loop: {}, constant volume: {}, divider: {}, volume: {}",
                 pulse.duty_cycle, Pulse::DUTY_CYCLES[pulse.duty_cycle as usize], pulse.length_counter.halt, pulse.envelope.loop_flag,
                 pulse.envelope.const_volume, pulse.envelope.divider, pulse.envelope.volume);

        Ok(())
    }

    fn write_pulse_sweep(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.sweep.enabled = (value & 0x80) != 0;
        pulse.sweep.period = ((value & 0x70) >> 4) + 1;
        pulse.sweep.negate = (value & 0x08) != 0;
        pulse.sweep.shift = value & 0x07;
        pulse.sweep.reload = true;

        trace!("APU: updated pulse sweep unit: enabled: {}, period: {}, negate: {}, shift: {}, reload: {}",
             pulse.sweep.enabled, pulse.sweep.period, pulse.sweep.negate, pulse.sweep.shift, pulse.sweep.reload);

        Ok(())
    }

    /***
     * 0x4002 and 0x4006 - pulse timer (period) low 8 bits
     *
     * https://www.nesdev.org/wiki/APU_Pulse
     ***/
    fn write_pulse_timer_lo(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.period = (pulse.period & 0xFF00) | value as u16;
        trace!("APU: updated pulse timer low byte: 0x{:04X}", pulse.period);

        Ok(())
    }

    /***
     * 0x4003 and 0x4007 - pulse length counter load and timer (period) high 3 bits
     *
     * https://www.nesdev.org/wiki/APU_Pulse
     ***/
    fn write_length_counter_and_timer_hi(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.period = pulse.period & 0x00FF | (value & 0x07) as u16;

        if pulse.enabled {
            pulse.length_counter.reload = LengthCounter::LENGTH_COUNTER_LOOKUP_TABLE[(value >> 3) as usize];
        }

        pulse.envelope.start_flag = true;
        pulse.duty_cycle_index = 0;

        trace!("APU: updated pulse timer high byte: {}, and length counter load: {}", pulse.period, pulse.length_counter.reload);

        Ok(())
    }

    /***
     * XXX
     * should clear the frame counter interrupt flag
     *
     * https://www.nesdev.org/wiki/APU#Status_($4015)
     ***/
    fn read_channels_status(&self) -> Result<u8, MemoryError> {
        let pulse1 = self.pulse1.length_counter.halt || self.pulse1.period == 0;
        let pulse2 = self.pulse2.length_counter.halt || self.pulse2.period == 0;

        let status = (pulse1 as u8) | ((pulse2 as u8) << 1);

        trace!("APU: channels status: pulse1: {}, pulse2: {}, status: 0x{:02X}",
             pulse1, pulse2, status);

        Ok(status)
    }

    fn write_channels_status(&mut self, value: u8) -> Result<(), MemoryError> {
        self.pulse1.enabled = (value & 0x01) != 0;
        self.pulse2.enabled = (value & 0x02) != 0;

        trace!("APU: updated channels status: pulse1 enabled: {}, pulse2 enabled: {}",
             self.pulse1.enabled, self.pulse2.enabled);

        Ok(())
    }

    fn write_frame_counter(&mut self, value: u8) -> Result<(), MemoryError> {
        self.frame_counter.mode = match value & 0x80 == 0 {
            true => FrameCounterMode::FourStep,
            false => FrameCounterMode::FiveStep,
        };

        self.frame_counter.inhibit_irq = (value & 0x40) != 0;

        trace!("APU: updated frame counter: mode: {:?}, inhibit_irq: {}",
             self.frame_counter.mode, self.frame_counter.inhibit_irq);

        Ok(())
    }

    fn clock_pulse_timers(&mut self) {
        let mut idx = 1;

        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            if pulse.enabled == false {
                continue
            }

            if pulse.period_counter == 0 {
                pulse.duty_cycle_index = (pulse.duty_cycle_index + 1) % 8;
                pulse.period_counter = pulse.period;

                trace!("APU: period counter for pulse channel {}: {} (period: {})", idx, pulse.period_counter, pulse.period);
                trace!("APU: cycle: {:?}, index: {}, position: {}",
                         Pulse::DUTY_CYCLES[pulse.duty_cycle], pulse.duty_cycle_index, Pulse::DUTY_CYCLES[pulse.duty_cycle][pulse.duty_cycle_index]);
            } else {
                pulse.period_counter -= 1;
            }

            idx += 1 ;
        }
    }

    fn convert_cpu_cycles_to_apu_cycles(cpu_cycle: u32) -> u32 {
        cpu_cycle / 2
    }

    fn clock_sweep_unit(sweep: &mut Sweep) {
        todo!();
    }

    fn clock_length_counter(length_counter: &mut LengthCounter) {
        if length_counter.halt == false && length_counter.counter > 0 {
            length_counter.counter -= 1;
        }
    }

    fn clock_envelope(envelop: &mut Envelope) {
        if envelop.start_flag {
            envelop.start_flag = false;
            envelop.volume = 15;
            envelop.divider = envelop.volume;
        } else {
            if envelop.divider > 0 {
                envelop.divider -= 1;
            } else {
                envelop.divider = envelop.volume;

                if envelop.counter > 0 {
                    envelop.counter -= 1;
                } else if envelop.loop_flag {
                    envelop.counter = 15;
                }
            }
        }
    }

    fn clock_sweep_units(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            ApuRp2A03::<T>::clock_sweep_unit(&mut pulse.sweep);
        }
    }

    fn clock_length_counters(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            ApuRp2A03::<T>::clock_length_counter(&mut pulse.length_counter);
        }
    }

    fn clock_envelopes(&mut self) {
        for pulse in [&mut self.pulse1, &mut self.pulse2] {
            ApuRp2A03::<T>::clock_envelope(&mut pulse.envelope);
        }
    }

    fn clock_frame_sequencer(&mut self, cycle: u32) {
        let step = FrameCounter::get_frame_sequencer_step_by_cycle(cycle);
        self.frame_counter.apu_cycle = cycle;

        match (&self.frame_counter.mode, step) {
            (_, 0) => {},
            (_, 1) => {
                self.clock_envelopes();
            },
            (_, 2) => {
                self.clock_envelopes();
                self.clock_length_counters();
                self.clock_sweep_units();
            },
            (_, 3) => {
                self.clock_envelopes();
            },
            (FrameCounterMode::FourStep, 4) => {
                self.clock_envelopes();
                self.clock_length_counters();
                self.clock_sweep_units();
                self.frame_counter.apu_cycle = 0;
            },
            (FrameCounterMode::FiveStep, 4) => {
                self.clock_envelopes();
            },
            (FrameCounterMode::FiveStep, 5) => {
                self.clock_envelopes();
                self.clock_length_counters();
                self.clock_sweep_units();
                self.frame_counter.apu_cycle = 0;
            },
            _ => unreachable!(),
        }
    }

    fn clock_mixer(&mut self) {
        let sample = self.pulse1.get_sample();
        self.samples[self.samples_index] = sample;
        self.samples_index = (self.samples_index + 1) % MAX_SAMPLING_BUFFER_SIZE;
    }
}

impl<T: SoundPlayback> APU for ApuRp2A03<T> {
    fn reset(&mut self) -> Result<(), ApuError> {
        info!("resetting APU");
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
        let mut apu_cycles_used = 0;
        let apu_credits = ApuRp2A03::<T>::convert_cpu_cycles_to_apu_cycles(credits);

        while apu_cycles_used < apu_credits {
            self.clock_pulse_timers();
            self.clock_frame_sequencer(apu_cycles_used);

            if self.last_mixer_cycle > MIXER_SAMPLING_CYCLE_THRESHOLD {
                self.clock_mixer();
                self.sound_player.playback();
                self.last_mixer_cycle += 1;
            } else {
                self.last_mixer_cycle = 0;
            }

            apu_cycles_used += 1;
        }

        Ok(credits)
    }
}