use log::{debug, info, trace};
use crate::apu::{ApuError, APU};
use crate::apu::ApuType::RP2A03;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};

const APU_NAME: &str = "APU RP2A03";
const APU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x4000, 0x4017);
const APU_EXTERNAL_MEMORY_SIZE: usize = 32;

#[derive(Debug)]
enum ChannelType {
    Pulse1,
    Pulse2
}

trait Channel {
}

#[derive(Debug)]
struct SweepUnit {
    enabled: bool,
    period: u8,
    shift: u8,
    negate: bool,
}

impl SweepUnit {
    fn new() -> Self {
        SweepUnit {
            enabled: false,
            period: 0,
            shift: 0,
            negate: false,
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
}

#[derive(Debug)]
struct LengthCounter {
    halt: bool,
    counter: u8,
    reload: u8
}

impl LengthCounter {
    fn new() -> Self {
        LengthCounter {
            halt: false,
            counter: 0,
            reload: 0,
        }
    }
}

#[derive(Debug)]
struct Pulse {
    enabled: bool,
    period: u8,
    timer: u16,
    duty: u8,
    duty_position: u8,
    sweep_unit: SweepUnit,
    envelope: Envelope,
    length_counter: LengthCounter,
}

impl Channel for Pulse {
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
            timer: 0,
            duty: 0,
            duty_position: 0,
            sweep_unit: SweepUnit::new(),
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
    inhibit_irq: bool
}

impl FrameCounter {
    fn new() -> Self {
        FrameCounter {
            mode: FrameCounterMode::FourStep,
            inhibit_irq: false
        }
    }
}

#[derive(Debug)]
pub struct ApuRp2A03 {
    pulse1: Pulse,
    pulse2: Pulse,
    frame_counter: FrameCounter,
}

impl BusDevice for ApuRp2A03 {
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

impl Memory for ApuRp2A03 {
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

impl ApuRp2A03 {
    pub fn new() -> Self {
        ApuRp2A03 {
            pulse1: Pulse::new(),
            pulse2: Pulse::new(),
            frame_counter: FrameCounter::new()
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

        pulse.duty = (value & 0xC0) >> 6;
        pulse.length_counter.halt = (value & 0x20) != 0;
        pulse.envelope.loop_flag = (value & 0x20) != 0;
        pulse.envelope.const_volume = (value & 0x10) != 0;
        pulse.envelope.divider = value & 0x0F;
        pulse.envelope.volume = value & 0x0F;

        println!("APU: updated pulse control: duty: {} ({:?}), length counter halt: {}, loop: {}, constant volume: {}, divider: {}, volume: {}",
             pulse.duty, Pulse::DUTY_CYCLES[pulse.duty as usize], pulse.length_counter.halt, pulse.envelope.loop_flag,
             pulse.envelope.const_volume, pulse.envelope.divider, pulse.envelope.volume);

        Ok(())
    }

    fn write_pulse_sweep(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.sweep_unit.enabled = (value & 0x80) != 0;
        pulse.sweep_unit.period = (value & 0x70) >> 4;
        pulse.sweep_unit.negate = (value & 0x08) != 0;
        pulse.sweep_unit.shift = value & 0x07;

        println!("APU: updated pulse sweep unit: enabled: {}, period: {}, negate: {}, shift: {}",
             pulse.sweep_unit.enabled, pulse.sweep_unit.period, pulse.sweep_unit.negate, pulse.sweep_unit.shift);

        Ok(())
    }

    fn write_pulse_timer_lo(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.timer = (pulse.timer & 0xFF00) | value as u16;
        println!("APU: updated pulse timer low byte: 0x{:04X}", pulse.timer);

        Ok(())
    }

    fn write_length_counter_and_timer_hi(&mut self, channel_type: ChannelType, value: u8) -> Result<(), MemoryError> {
        let pulse = self.get_pulse_channel_by_type(&channel_type);

        pulse.period = (value & 0xF8) >> 3;
        pulse.timer = (pulse.timer & 0x00FF) | (((value & 0x07) as u16) << 8);

        println!("APU: updated pulse timer period: {}, and timer high byte: {}", pulse.period, pulse.timer);

        Ok(())
    }

    /***
     * XXX
     * should clear the frame counter interrupt flag
     * https://www.nesdev.org/wiki/APU#Status_($4015)
     ***/
    fn read_channels_status(&self) -> Result<u8, MemoryError> {
        let pulse1 = self.pulse1.length_counter.halt || self.pulse1.period == 0;
        let pulse2 = self.pulse2.length_counter.halt || self.pulse2.period == 0;

        let status = (pulse1 as u8) | ((pulse2 as u8) << 1);

        println!("APU: channels status: pulse1: {}, pulse2: {}, status: 0x{:02X}",
             pulse1, pulse2, status);

        Ok(status)
    }

    fn write_channels_status(&mut self, value: u8) -> Result<(), MemoryError> {
        self.pulse1.enabled = (value & 0x01) != 0;
        self.pulse2.enabled = (value & 0x02) != 0;

        println!("APU: updated channels status: pulse1 enabled: {}, pulse2 enabled: {}",
             self.pulse1.enabled, self.pulse2.enabled);

        Ok(())
    }

    fn write_frame_counter(&mut self, value: u8) -> Result<(), MemoryError> {
        self.frame_counter.mode = match value & 0x80 == 0 {
            true => FrameCounterMode::FourStep,
            false => FrameCounterMode::FiveStep,
        };

        self.frame_counter.inhibit_irq = (value & 0x40) != 0;

        println!("APU: updated frame counter: mode: {:?}, inhibit_irq: {}",
             self.frame_counter.mode, self.frame_counter.inhibit_irq);

        Ok(())
    }
}

impl APU for ApuRp2A03 {
    fn reset(&mut self) -> Result<(), ApuError> {
        info!("resetting APU");
        Ok(())
    }

    fn panic(&self, _: &ApuError) {
        unreachable!()
    }

    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, ApuError> {
        Ok(0)
    }
}