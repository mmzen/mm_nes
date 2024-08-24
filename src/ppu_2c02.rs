use std::cell::RefCell;
use std::fmt::{Debug, Formatter};
use std::rc::Rc;
use log::{debug, info, trace};
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cpu::CPU;
use crate::dma_device::DmaDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::nes_bus::NESBus;
use crate::palette::Palette;
use crate::palette_2c02::Palette2C02;
use crate::ppu::{PPU, PpuError, PpuNameTableMirroring, PpuType};
use crate::ppu_2c02::ControlFlag::{BackgroundPatternTableAddr, BaseNameTableAddr1, BaseNameTableAddr2, GenerateNmi, VramIncrement};
use crate::ppu_2c02::PpuFlag::{Control, Mask, Status};
use crate::ppu_2c02::StatusFlag::VBlank;
use crate::renderer::Renderer;

const PPU_NAME: &str = "PPU 2C02";
//const NAME_TABLE_HORIZONTAL_ADDRESS_SPACE: [(u16, u16); 2] = [(0x2000, 0x27FF), (0x2800, 0x2FFF)];
const NAME_TABLE_HORIZONTAL_ADDRESS_SPACE: [(u16, u16); 2] = [(0x2000, 0x27FF), (0x2800, 0x3EFF)];
const NAME_TABLE_VERTICAL_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x3EFF);
const NAME_TABLE_HORIZONTAL_SIZE: usize = 1024;
const NAME_TABLE_VERTICAL_SIZE: usize = 2048;
const NAME_TABLE_SIZE: u16 = 960;
const PALETTE_ADDRESS_SPACE: (u16, u16) = (0x3F00, 0x3FFF);
const PALETTE_SIZE: usize = 32;
const V_INCR_GOING_ACROSS: u8 = 1;
const V_INCR_GOING_DOWN: u8 = 32;
const PPU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x3FFF);
const PPU_EXTERNAL_MEMORY_SIZE: usize = 8;
const PATTERN_TABLE_LEFT_ADDR: u16 = 0x0000;
const PATTERN_TABLE_RIGHT_ADDR: u16 = 0x1000;
const TILE_X_MAX: usize = 32;
const TILE_Y_MAX: usize = 30;
const PATTERN_DATA_SIZE: usize = 16;


#[derive(Debug)]
enum PpuFlag {
    Control(ControlFlag),
    Mask(MaskFlag),
    Status(StatusFlag)
}

impl PpuFlag {
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
struct Latch {
    state: LatchState
}

impl Latch {

    fn new() -> Self {
        Latch {
            state: LatchState::HIGH
        }
    }

    fn latch(&mut self) {
        self.state = match self.state {
            LatchState::HIGH => LatchState::LOW,
            LatchState::LOW => LatchState::HIGH,
        };
    }

    fn reset(&mut self) {
        self.state = LatchState::HIGH;
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
    v: RefCell<u16>,
    latch: RefCell<Latch>,
    renderer: Renderer,
    cpu: Rc<RefCell<dyn CPU>>,
}

impl PPU for Ppu2c02 {
    fn reset(&mut self) -> Result<(), PpuError> {
        info!("resetting PPU");

        self.register.borrow_mut().control = 0;
        self.register.borrow_mut().mask = 0;
        self.register.borrow_mut().status = 0;
        self.register.borrow_mut().scroll = 0;
        self.register.borrow_mut().data = 0;

        self.latch.borrow_mut().reset();
        *self.v.borrow_mut() = 0;

        self.set_flag(Status(VBlank), true);

        Ok(())
    }

    fn panic(&self, _: &PpuError) {
        todo!()
    }

    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, PpuError> {
        let mut cycles = start_cycle;
        let cycles_threshold = start_cycle + credits;

        debug!("running PPU - cycle: {}, credits: {}, threshold: {}", start_cycle, credits, cycles_threshold);

        loop {
            cycles = cycles + self.render()?;

            if cycles >= cycles_threshold {
                break;
            }
        }

        Ok(cycles)
    }
}

impl Memory for Ppu2c02 {

    fn initialize(&mut self) -> Result<usize, MemoryError> {
        info!("initializing PPU");
        let _ = self.reset();
        Ok(PPU_EXTERNAL_MEMORY_SIZE)
    }

    fn read_byte(&self, addr: u16) -> Result<u8, MemoryError> {
        trace!("reading byte at 0x{:04X}", addr);

        let value = match addr {
            0x00 => self.read_control_register(),
            0x01 => self.read_mask_register(),
            0x02 => self.read_status_register(),
            0x03 => self.read_oam_address_register(),
            0x04 => self.read_oam_data_register(self.register.borrow().oam_addr),
            0x05 => self.read_scroll_register(),
            0x06 => self.read_addr_register(),
            0x07 => self.read_data_register()?,
            _ => unreachable!(),
        };

        trace!("read byte at 0x{:04X}: {:02X}", addr, value);
        Ok(value)
    }

    fn trace_read_byte(&self, addr: u16) -> Result<u8, MemoryError> {

        let value = match addr {
            0x00 => self.register.borrow().control,
            0x01 => self.register.borrow().mask,
            0x02 => self.register.borrow().status,
            0x03 => self.register.borrow().oam_addr,
            0x04 => self.read_oam_data_register(self.register.borrow().oam_addr),
            0x05 => self.register.borrow().scroll,
            0x06 => {
                if self.latch.borrow().state == LatchState::HIGH {
                    (*self.v.borrow() >> 8) as u8
                } else {
                    *self.v.borrow() as u8
                }
            },
            0x07 => self.register.borrow().data,
            _ => unreachable!(),
        };

        Ok(value)
    }

    fn write_byte(&mut self, addr: u16, value: u8) -> Result<(), MemoryError> {
        trace!("writing byte ({:02X}) at 0x{:04X}", value, addr);

        match addr {
            0x00 => self.write_control_register(value),
            0x01 => self.write_mask_register(value),
            0x02 => self.write_status_register(value),
            0x03 => self.write_oam_address_register(value),
            0x04 => {
                let addr = self.register.borrow().oam_addr;
                self.write_oam_data_register(addr, value)
            },
            0x05 => self.write_scroll_register(value),
            0x06 => self.write_addr_register(value),
            0x07 => self.write_data_register(value)?,
            _ => unreachable!(),
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

impl DmaDevice for Ppu2c02 {
    fn dma_write(&mut self, addr: u8, value: u8) -> Result<(), MemoryError> {
        self.write_oam_data_register(addr, value);
        Ok(())
    }
}

impl Ppu2c02 {

    fn v_wrapping_add(&self, n: u16) -> u16 {
        let mut v = self.v.borrow().wrapping_add(n);

        if v < PPU_EXTERNAL_ADDRESS_SPACE.0 {
            v = PPU_EXTERNAL_ADDRESS_SPACE.0 + (v % 0x1000);
        } else if v > PPU_EXTERNAL_ADDRESS_SPACE.1 {
            v = PPU_EXTERNAL_ADDRESS_SPACE.0 + (v - (PPU_EXTERNAL_ADDRESS_SPACE.1 + 1));
        }

        v
    }

    fn read_control_register(&self) -> u8 {
        self.register.borrow().control
    }

    fn write_control_register(&mut self, value: u8) {
        self.register.borrow_mut().control = value;
    }

    fn read_mask_register(&self) -> u8 {
        self.register.borrow().mask
    }

    fn write_mask_register(&mut self, value: u8) {
        self.register.borrow_mut().mask = value;
    }

    fn read_status_register(&self) -> u8 {
        let result = self.register.borrow().status;
        self.set_flag(Status(VBlank), false);
        self.latch.borrow_mut().reset();

        result
    }

    fn write_status_register(&mut self, value: u8) {
        self.register.borrow_mut().status = value;
    }

    fn read_oam_address_register(&self) -> u8 {
        self.register.borrow().oam_addr
    }

    fn write_oam_address_register(&mut self, value: u8) {
        self.register.borrow_mut().oam_addr = value;
    }

    fn read_oam_data_register(&self, addr: u8) -> u8 {
        let sprite_index = (addr / 4) as usize;
        let offset = addr % 4;

        match offset {
            0 => self.oam[sprite_index].y,
            1 => self.oam[sprite_index].tile_number,
            2 => self.oam[sprite_index].attributes,
            3 => self.oam[sprite_index].x,
            _ => unreachable!(),
        }
    }

    fn write_oam_data_register(&mut self, addr: u8, value: u8) {
        let sprite_index = (addr / 4) as usize;
        let offset = addr % 4;

        match offset {
            0 => self.oam[sprite_index].y = value,
            1 => self.oam[sprite_index].tile_number = value,
            2 => self.oam[sprite_index].attributes = value,
            3 => self.oam[sprite_index].x = value,
            _ => unreachable!(),
        }
    }

    fn read_scroll_register(&self) -> u8 {
        self.register.borrow().scroll
    }

    fn write_scroll_register(&mut self, value: u8) {
        self.register.borrow_mut().scroll = value;
    }

    fn read_addr_register(&self) -> u8 {
        let v = *self.v.borrow();

        match self.latch.borrow().state {
            LatchState::LOW => v as u8,
            LatchState::HIGH => (v >> 8) as u8,
        }
    }

    fn write_addr_register(&mut self, value: u8) {
        let mut v = *self.v.borrow();

        v = match self.latch.borrow().state {
            LatchState::LOW => (v & 0xFF00) | (value as u16),
            LatchState::HIGH => (v & 0x00FF) | (value as u16) << 8
        };

        *self.v.borrow_mut() = v;
        self.latch.borrow_mut().latch();
    }

    fn read_data_register(&self) -> Result<u8, MemoryError> {
        let previous_read = self.register.borrow().data;
        let video_addr = *self.v.borrow();
        let incr = self.get_v_increment_value() as u16;
        let incremented_v = self.v_wrapping_add(incr);

        self.register.borrow_mut().data = self.bus.read_byte(video_addr)?;
        *self.v.borrow_mut() = incremented_v;

        Ok(previous_read)
    }

    fn write_data_register(&mut self, value: u8) -> Result<(), MemoryError> {
        let incr = self.get_v_increment_value() as u16;
        let incremented_v = self.v_wrapping_add(incr);

        //debug!("write through PPU data register: 0x{:02X} to 0x{:04X}", value, *self.v.borrow());
        self.bus.write_byte(*self.v.borrow(), value)?;

        *self.v.borrow_mut() = incremented_v;
        //debug!("write through PPU data register: v is now 0x{:04X}", *self.v.borrow());
        Ok(())
    }

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

    pub fn new(chr_rom: Rc<RefCell<dyn BusDevice>>, mirroring: PpuNameTableMirroring, cpu: Rc<RefCell<dyn CPU>>) -> Result<Self, PpuError> {
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
            v: RefCell::new(0),
            oam: vec![SpriteDisplay::default(); 64],
            latch: RefCell::new(Latch::new()),
            renderer: Renderer::new(),
            cpu,
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
            "addr" => *self.v.borrow() as u8,
            "data" => self.register.borrow().data,
            _ => 0,
        }
    }

    #[cfg(test)]
    pub fn get_v_value(&self) -> u16 {
        *self.v.borrow()
    }

    #[cfg(test)]
    pub fn ext_set_flag(&mut self, flag: PpuFlag, value: bool) {
        self.set_flag(flag, value);
    }


    fn set_flag(&self, flag: PpuFlag, value: bool) {
        let p = match flag {
            Control(_) => {
                &mut self.register.borrow_mut().control
            },

            Mask(_) => {
                &mut self.register.borrow_mut().mask
            },

            Status(_) => {
                &mut self.register.borrow_mut().status
            }
        };

        if value {
            *p |= flag.bits()
        } else {
            *p &= !flag.bits()
        }
    }

    #[cfg(test)]
    pub fn ext_get_flag(&mut self, flag: PpuFlag) {
        self.get_flag(flag);
    }


    fn get_flag(&self, flag: PpuFlag) -> bool {
        match flag {
            Control(_) => (self.register.borrow_mut().control & flag.bits()) != 0,
            Mask(_) => (self.register.borrow_mut().mask & flag.bits()) != 0,
            Status(_) => (self.register.borrow_mut().status & flag.bits()) != 0
        }
    }

    fn get_v_increment_value(&self) -> u8 {
        match self.get_flag(Control(VramIncrement)) {
            true => V_INCR_GOING_DOWN,
            false => V_INCR_GOING_ACROSS,
        }
    }

    fn fetch_palette(&self, tile_x: usize, tile_y: usize, attribute_table_addr: u16) -> Result<u8, PpuError> {
        let block_x = tile_x / 4;
        let block_y = tile_y / 4;
        let attribute_table_address = attribute_table_addr + (block_y as u16 * 8) + block_x as u16;
        let attribute_data = self.bus.read_byte(attribute_table_address)?;

        let quadrant_x = (tile_x % 4) / 2;
        let quadrant_y = (tile_y % 4) / 2;
        let shift = 2 * (quadrant_y * 2 + quadrant_x);
        let palette = (attribute_data >> shift ) & 0x03;

        trace!("attribute_table_address: 0x{:04X}, palette: 0x{:02X}", attribute_table_address, palette);
        Ok(palette)
    }

    fn fetch_line_pattern_data(&self, pattern_table_addr: u16, tile_index: u8, line: usize) -> Result<Vec<u8>, PpuError> {
        let mut line_pattern_data= vec![0u8; 8];
        let line = line as u16;
        let tile_index = tile_index as u16;

        let pattern_data0 = self.bus.read_byte(pattern_table_addr + (tile_index * PATTERN_DATA_SIZE as u16) + line)?;
        let pattern_data1 = self.bus.read_byte(pattern_table_addr + (tile_index * PATTERN_DATA_SIZE as u16) + line + (PATTERN_DATA_SIZE as u16 / 2))?;

        for bit in 0..=7 {
            let value0 = (pattern_data0 >> 7 - bit) & 0x01;
            let value1 = (pattern_data1 >> 7 - bit) & 0x01;
            let combined = (value1 << 1) | value0;
            line_pattern_data[bit] = combined;
        }

        trace!("line_pattern_data: {:?}", line_pattern_data);
        Ok(line_pattern_data)
    }

    fn fetch_tile_index(&self, tile_x: usize, tile_y: usize, base_name_table_addr: u16) -> Result<u8, PpuError> {
        let name_table_index = (tile_x + (tile_y * 32)) as u16;
        let addr = base_name_table_addr + name_table_index;
        let tile_index = self.bus.read_byte(addr)?;

        trace!("tile_index at 0x{:04X} in name table: 0x{:02X}", addr, tile_index);
        Ok(tile_index)
    }

    fn get_palette_address(&self, palette: u8) -> u16 {
        let palette_address = PALETTE_ADDRESS_SPACE.0 + (palette as u16 * 4);

        trace!("palette address: 0x{:04X}", palette_address);
        palette_address
    }

    fn get_palette_color(&self, palette: u8) -> Result<(u8, u8, u8, u8), PpuError> {
        let mut colors = [0u8; 4];

        let palette_address = self.get_palette_address(palette);
        for i in 0..=3 {
            colors[i] = self.bus.read_byte(palette_address + i as u16)?;
        }

        trace!("palette color: (0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X})", colors[0], colors[1], colors[2], colors[3]);
        Ok((colors[0], colors[1], colors[2], colors[3]))
    }

    fn set_pixels(&mut self, tile_x: usize, tile_y: usize, line: usize, line_pattern_data: Vec<u8>, colors: (u8, u8, u8, u8)) {
        for pixel in 0..=7 {
            let color = line_pattern_data[pixel];
            trace!("x: {}, y: {}, color: {}, palette: {:?}", pixel + (tile_x * 8), line + (tile_y * 8), color, colors);

            let rgb: (u8, u8, u8) = match color {
                0 => Palette2C02::rgb(colors.0),
                1 => Palette2C02::rgb(colors.1),
                2 => Palette2C02::rgb(colors.2),
                3 => Palette2C02::rgb(colors.3),
                _ => unreachable!()
            };
            self.renderer
                .frame()
                .set_pixel(pixel + (tile_x * 8), line + (tile_y * 8), rgb);
        }
    }

    fn get_pattern_table_addr(&self) -> u16 {
        let pattern_table_addr = if self.get_flag(Control(BackgroundPatternTableAddr)) {
            PATTERN_TABLE_RIGHT_ADDR
        } else {
            PATTERN_TABLE_LEFT_ADDR
        };

        trace!("pattern table address: 0x{:04X}", pattern_table_addr);
        pattern_table_addr
    }

    fn get_attribute_table_addr(&self, base_name_table_addr: u16) -> u16 {
        let attribute_table_addr= base_name_table_addr + NAME_TABLE_SIZE;

        trace!("attribute table: 0x{:04X}", attribute_table_addr);
        attribute_table_addr
    }

    fn get_name_table_addr(&self) -> u16 {
        let base_name_table_addr_status = (self.get_flag(Control(BaseNameTableAddr2)) as u8) << 1 | (self.get_flag(Control(BaseNameTableAddr1)) as u8);
        let base_name_table_addr = match base_name_table_addr_status {
            0x00 => NAME_TABLE_HORIZONTAL_ADDRESS_SPACE[0].0,
            0x01 => NAME_TABLE_HORIZONTAL_ADDRESS_SPACE[0].0 + NAME_TABLE_HORIZONTAL_SIZE as u16,
            0x02 => NAME_TABLE_HORIZONTAL_ADDRESS_SPACE[1].0,
            0x03 => NAME_TABLE_HORIZONTAL_ADDRESS_SPACE[0].0 + NAME_TABLE_HORIZONTAL_SIZE as u16,
            _ => unreachable!(),
        };

        trace!("base name table: 0x{:04X}", base_name_table_addr);
        base_name_table_addr
    }

    fn render(&mut self) -> Result<u32, PpuError> {
        debug!("starting rendering");

        self.set_flag(Status(VBlank), false);
        debug!("control register: 0x{:02X}", self.register.borrow().control);

        let base_name_table_addr = self.get_name_table_addr();
        let attribute_table_addr = self.get_attribute_table_addr(base_name_table_addr);
        let pattern_table_addr = self.get_pattern_table_addr();

        for tile_y in 0..TILE_Y_MAX {
            for tile_x in 0..TILE_X_MAX {
                let tile_index = self.fetch_tile_index(tile_x, tile_y, base_name_table_addr)?;
                let palette = self.fetch_palette(tile_x, tile_y, attribute_table_addr)?;
                let colors = self.get_palette_color(palette)?;

                for line in 0..=7usize {
                    let line_pattern_data = self.fetch_line_pattern_data(pattern_table_addr, tile_index, line)?;
                    self.set_pixels(tile_x, tile_y, line, line_pattern_data, colors);
                }
            }
        }

        self.renderer.update();
        info!("rendering done");

        self.set_flag(Status(VBlank), true);

        if self.get_flag(Control(GenerateNmi)) {
            self.cpu.borrow_mut().signal_nmi()?;
        }

        Ok(300)
    }
}