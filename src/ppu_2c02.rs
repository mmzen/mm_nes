use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use log::{debug, info};
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::ppu::{PPU, PpuError, PpuNameTableMirroring, PpuType};
use crate::ppu_2c02::ControlFlag::VramIncrement;
use crate::ppu_2c02::PPUFlag::{Control, Mask, Status};

const PPU_NAME: &str = "PPU 2C02";
const CHR_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x1FFF);
const NAME_TABLE_HORIZONTAL_ADDRESS_SPACE: [(u16, u16); 2] = [(0x2000, 0x23FF), (0x2400, 0x27FF)];
const NAME_TABLE_VERTICAL_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x27FF);
const NAME_TABLE_HORIZONTAL_SIZE: usize = 1024;
const NAME_TABLE_VERTICAL_SIZE: usize = 2048;
const PALETTE_ADDRESS_SPACE: (u16, u16) = (0x3F00, 0x3FFF);
const PALETTE_SIZE: usize = 32;
const V_INCR_GOING_ACROSS: u16 = 1;
const V_INCR_GOING_DOWN: u16 = 32;
const PPU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x3FFF);
const PPU_EXTERNAL_MEMORY_SIZE: usize = 8;

enum PPUFlag {
    Control(ControlFlag),
    Mask(MaskFlag),
    Status(StatusFlag)
}

impl PPUFlag {
    fn bits(&self) -> u8 {
        match self {
            Control(flag) => *flag as u8,
            Mask(flag) => *flag as u8,
            Status(flag) => *flag as u8
        }
    }
}

#[derive(Debug, Copy, Clone)]
enum ControlFlag {
    BaseNameTableAddr1 = 0x01,
    BaseNameTableAddr2 = 0x02,
    VramIncrement = 0x04,
    SpritePatternAddr = 0x08,
    BackgroundPatternTableAddr = 0x10,
    SpriteSize = 0x20,
    MasterSlaveSelect = 0x40,
    GenerateNmi = 0x80
}

#[derive(Debug, Copy, Clone)]
enum MaskFlag {
    GreyScale = 0x01,
    ShowLeftmostBackground = 0x02,
    ShowLeftmostSprites = 0x04,
    ShowBackground = 0x08,
    ShowSprites = 0x10,
    EmphasizeRed = 0x20,
    EmphasizeGreen = 0x40,
    EmphasizeBlue = 0x80,
}

#[derive(Debug, Copy, Clone)]
enum StatusFlag {
    StaleOpenBus = 0x1F,
    SpriteOverflow = 0x20,
    Sprite0Hit = 0x40,
    VBlank = 0x80,
}

#[derive(Debug, PartialEq)]
enum LatchState {
    HIGH,
    LOW
}

#[derive(Debug)]
struct LatchRegister {
    value: u16,
    latch: LatchState
}

impl LatchRegister {

    fn new() -> Self {
        LatchRegister {
            value: 0,
            latch: LatchState::HIGH
        }
    }

    fn latch(&mut self) {
        if self.latch == LatchState::LOW {
            self.latch = LatchState::HIGH
        } else {
            self.latch = LatchState::LOW;
        }
    }

    fn increment(&mut self, value: u16) -> u16 {
        self.value.wrapping_add(value)
    }

    fn write(&mut self, value: u8) {
        match self.latch {
            LatchState::LOW => {
                self.value = (self.value & 0xFF00) | (value as u16);
            },
            LatchState::HIGH => {
                self.value = (self.value & 0x00FF) | (value as u16) << 8 ;
            }
        }
        self.latch();
    }

    fn read(&self) -> u8 {
        match self.latch {
            LatchState::LOW => {
                (self.value & 0x00FF) as u8
            },
            LatchState::HIGH => {
                (self.value & 0xFF00) as u8
            }
        }
    }
}

#[derive(Debug)]
struct Register {
    control: u8,
    mask: u8,
    status: u8,
    oam_addr: u8,
    oam_data: u8,
    scroll: u8,
    data: u8
}

impl Register {
    fn new() -> Self {
        Register {
            control: 0,
            mask: 0,
            status: 0,
            oam_addr: 0,
            oam_data: 0,
            scroll: 0,
            data: 0
        }
    }
}

#[derive(Debug, Clone)]
struct SpriteDisplay {
    x: u8,
    y: u8,
    tile_number: u8,
    attributes: u8,
    pattern_table_index: u8
}

impl Default for SpriteDisplay {
    fn default() -> Self {
        SpriteDisplay {
            x: 0,
            y: 0,
            tile_number: 0,
            attributes: 0,
            pattern_table_index: 0
        }
    }
}

pub struct Ppu2c02 {
    register: RefCell<Register>,
    bus: Box<dyn Bus>,
    oam: Vec<SpriteDisplay>,
    v: RefCell<LatchRegister>
}

impl PPU for Ppu2c02 {
    fn reset(&mut self) -> Result<(), PpuError> {
        self.register.borrow_mut().control = 0;
        self.register.borrow_mut().mask = 0;
        self.register.borrow_mut().scroll = 0;
        self.register.borrow_mut().data = 0;

        self.v.borrow_mut().write(0);
        Ok(())
    }

    fn panic(&self, _: &PpuError) {
        todo!()
    }
}

impl Memory for Ppu2c02 {
    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing PPU");
        Ok(PPU_EXTERNAL_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        let effective_addr = PPU_EXTERNAL_ADDRESS_SPACE.0 + (addr & (PPU_EXTERNAL_MEMORY_SIZE as u16 - 1));

        let value = match effective_addr {
            0x2000 => self.register.borrow().control,
            0x2001 => self.register.borrow().mask,
            0x2002 => self.register.borrow().status,
            0x2003 => self.register.borrow().oam_addr,
            0x2004 => {
                let sprite_index = (self.register.borrow().oam_addr / 4) as usize;
                let offset = self.register.borrow().oam_addr % 4;

                match offset {
                    0 => self.oam[sprite_index].y,
                    1 => self.oam[sprite_index].tile_number,
                    2 => self.oam[sprite_index].attributes,
                    3 => self.oam[sprite_index].x,
                    _ => unreachable!(),
                }
            },
            0x2005 => self.register.borrow().scroll,
            0x2006 => {
                self.v.borrow().read()
            },
            0x2007 => {
                let previous_read = self.register.borrow().data;
                let video_addr = self.v.borrow().value;
                let incr = self.get_v_increment_value();

                self.register.borrow_mut().data = self.bus.read_byte(video_addr)?;
                self.v.borrow_mut().increment(incr);

                previous_read
            },
            _ => return Err(MemoryError::OutOfRange(effective_addr)),
        };

        Ok(value)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        let effective_addr = PPU_EXTERNAL_ADDRESS_SPACE.0 + (addr & (PPU_EXTERNAL_MEMORY_SIZE as u16 - 1));

        match effective_addr {
            0x2000 => self.register.borrow_mut().control = value,
            0x2001 => self.register.borrow_mut().mask = value,
            0x2002 => self.register.borrow_mut().status = value,
            0x2003 => self.register.borrow_mut().oam_addr = value,
            0x2004 => {
                let sprite_index = (self.register.borrow().oam_addr / 4) as usize;
                let offset = self.register.borrow().oam_addr % 4;

                match offset {
                    0 => self.oam[sprite_index].y = value,
                    1 => self.oam[sprite_index].tile_number = value,
                    2 => self.oam[sprite_index].attributes = value,
                    3 => self.oam[sprite_index].x = value,
                    _ => unreachable!(),
                }
            }
            0x2005 => self.register.borrow_mut().scroll = value,
            0x2006 => {
                self.v.borrow_mut().write(value);
            },
            0x2007 => {
                let incr = self.get_v_increment_value();

                self.register.borrow_mut().data = value;
                self.bus.write_byte(self.v.borrow().value, value)?;
                self.v.borrow_mut().increment(incr);
            },
            _ => return Err(MemoryError::OutOfRange(effective_addr)),
        };

        Ok(())
    }

    fn read_word(&self, addr: u16) -> Result<u16, MemoryError> {
        Err(MemoryError::OutOfRange(addr))
    }

    fn write_word(&mut self, addr: u16, _: u16) -> Result<(), MemoryError> {
        Err(MemoryError::OutOfRange(addr))
    }

    fn dump(&self) {
        todo!()
    }

    fn size(&self) -> usize {
        PPU_EXTERNAL_MEMORY_SIZE
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
        BusDeviceType::PPU(PpuType::NES2C02)
    }

    fn get_address_range(&self) -> (u16, u16) {
        PPU_EXTERNAL_ADDRESS_SPACE
    }

    fn is_addr_in_address_space(&self, addr: u16) -> bool {
        PPU_EXTERNAL_ADDRESS_SPACE.0 <= addr && addr <= PPU_EXTERNAL_ADDRESS_SPACE.1
    }
}

impl Ppu2c02 {

    fn create_mirrored_name_tables_and_connect_to_bus(bus: &mut Box<dyn Bus>, mirroring: PpuNameTableMirroring) -> Result<(), PpuError> {
        debug!("setting name tables to mirroring mode: {:?}", mirroring);

        match mirroring {
            PpuNameTableMirroring::Vertical => {
                let memory = Rc::new(RefCell::new(
                    MemoryBank::new(NAME_TABLE_VERTICAL_SIZE, NAME_TABLE_VERTICAL_ADDRESS_SPACE)));
                memory.borrow_mut().initialize()?;
                bus.add_device(memory)?;
            },

            PpuNameTableMirroring::Horizontal => {
                for &(start, end) in &NAME_TABLE_HORIZONTAL_ADDRESS_SPACE {
                    let memory = Rc::new(RefCell::new(
                        MemoryBank::new(NAME_TABLE_HORIZONTAL_SIZE, (start, end))));
                    memory.borrow_mut().initialize()?;
                    bus.add_device(memory)?;
                }
            }
        }
        Ok(())
    }

    pub fn new(chr_rom: Rc<RefCell<dyn BusDevice>>, mirroring: PpuNameTableMirroring) -> Result<Self, PpuError> {
        let mut bus: Box<dyn Bus> = Box::new(NESBus::new());

        let palette_table = Rc::new(RefCell::new(
            MemoryBank::new(PALETTE_SIZE, PALETTE_ADDRESS_SPACE)));

        palette_table.borrow_mut().initialize()?;

        bus.add_device(palette_table)?;
        bus.add_device(chr_rom)?;

        Ppu2c02::create_mirrored_name_tables_and_connect_to_bus(&mut bus, mirroring)?;

        let ppu = Ppu2c02 {
            register: RefCell::new(Register::new()),
            bus,
            v: RefCell::new(LatchRegister::new()),
            oam: vec![SpriteDisplay::default(); 64]
        };

        Ok(ppu)
    }

    #[cfg(test)]
    pub fn get_register_value(&self, name: &str) -> u8 {
        match name {
            "controller" => self.register.borrow().control,
            "mask" => self.register.borrow().mask,
            "status" => self.register.borrow().status,
            "oam_addr" => self.register.borrow().oam_addr,
            "oam_data" => self.register.borrow().oam_data,
            "scroll" => self.register.borrow().scroll,
            "addr" => self.v.borrow().read(),
            "data" => self.register.borrow().data,
            _ => 0,
        }
    }

    #[cfg(test)]
    pub fn get_v_value(&self) -> u16 {
        self.v.borrow().value
    }

    fn set_flag(&mut self, flag: PPUFlag) {
        match flag {
            Control(_) => {
                self.register.borrow_mut().control |= flag.bits();
            },

            Mask(_) => {
                self.register.borrow_mut().mask |= flag.bits();
            },

            Status(_) => {
                self.register.borrow_mut().status |= flag.bits();
            }
        }
    }

    fn get_flag(&self, flag: PPUFlag) -> bool {
        match flag {
            Control(_) => {
                (self.register.borrow_mut().control & flag.bits()) != 0
            },

            Mask(_) => {
                (self.register.borrow_mut().mask & flag.bits()) != 0
            },

            Status(_) => {
                (self.register.borrow_mut().status & flag.bits()) != 0
            }
        }
    }

    fn get_v_increment_value(&self) -> u16 {
        match self.get_flag(Control(VramIncrement)) {
            false => V_INCR_GOING_ACROSS,
            true => V_INCR_GOING_DOWN,
        }
    }
}