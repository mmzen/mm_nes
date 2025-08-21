use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use std::time::Duration;
use log::{debug, info, trace};
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::cpu::CPU;
use crate::dma_device::DmaDevice;
use crate::memory::{Memory, MemoryError};
use crate::memory_bank::MemoryBank;
use crate::memory_mirror::MemoryMirror;
use crate::memory_palette::MemoryPalette;
use crate::nes_bus::NESBus;
use crate::palette::Palette;
use crate::palette_2c02::Palette2C02;
use crate::ppu::{PPU, PpuError, PpuNameTableMirroring, PpuType};
use crate::ppu_2c02::ControlFlag::{BackgroundPatternTableAddr, GenerateNmi, SpritePatternTableAddr, SpriteSize, VramIncrement};
use crate::ppu_2c02::MaskFlag::{ShowBackground, ShowSprites};
use crate::ppu_2c02::PpuFlag::{Control, Mask, Status};
use crate::ppu_2c02::SpriteAttribute::{FlipHorizontal, FlipVertical};
use crate::ppu_2c02::StatusFlag::{Sprite0Hit, SpriteOverflow, VBlank};
use crate::renderer::Renderer;
use crate::util::{measure_exec_time, vec_to_array};

const PPU_NAME: &str = "PPU 2C02";

pub const NT_BASES: [(u16,u16); 8] = [
    (0x2000, 0x23FF), // name table 1
    (0x2400, 0x27FF), // name table 2
    (0x2800, 0x2BFF), // name table 3
    (0x2C00, 0x2FFF), // name table 4
    (0x3000, 0x33FF), // mirror of name table 1
    (0x3400, 0x37FF), // mirror of name table 2
    (0x3800, 0x3BFF), // mirror of name table 3
    (0x3C00, 0x3EFF), // partial mirror of name table 4
];

pub const NT_MAP_HORIZONTAL: [usize; 8] = [0, 0, 1, 1, 0, 0, 1, 1]; // A and B same name table; C and D same name table
pub const NT_MAP_VERTICAL:   [usize; 8] = [0, 1, 0, 1, 0, 1, 0, 1]; // A and C same name table; B and D same name table
const NAME_TABLE_SIZE: usize = 1024;
const ATTRIBUTE_TABLE_SIZE: usize = 64;
const PATTERN_TABLE_LEFT_ADDR: u16 = 0x0000;
const PATTERN_TABLE_RIGHT_ADDR: u16 = 0x1000;

const PALETTE_ADDRESS_SPACE: (u16, u16) = (0x3F00, 0x3FFF);
const SPRITE_PALETTE_ADDR: u16 = 0x3F10;
const PALETTE_SIZE: usize = 32;

const V_INCR_GOING_ACROSS: u8 = 1;
const V_INCR_GOING_DOWN: u8 = 32;

const PPU_EXTERNAL_ADDRESS_SPACE: (u16, u16) = (0x2000, 0x3FFF);
const PPU_EXTERNAL_MEMORY_SIZE: usize = 8;
const PPU_INTERNAL_ADDRESS_SPACE: (u16, u16) = (0x0000, 0x3FFF);


const PIXEL_X_MAX: u8 = 255;
const PIXEL_Y_MAX: u8 = 239;
const SPRITE_WIDTH: u8 = 8;
const PATTERN_DATA_SIZE: usize = 16;
const MERGED_PATTERN_DATA_SIZE: usize = 64;

const CLOCK_CYCLES_PER_SCANLINE: u16 = 114;


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

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
enum ControlFlag {
    BaseNameTableAddr1 = 0x01,
    BaseNameTableAddr2 = 0x02,
    VramIncrement = 0x04,
    SpritePatternTableAddr = 0x08,
    BackgroundPatternTableAddr = 0x10,
    SpriteSize = 0x20,
    MasterSlaveSelect = 0x40,
    GenerateNmi = 0x80
}

#[allow(dead_code)]
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

#[allow(dead_code)]
#[derive(Debug, Copy, Clone)]
enum SpriteAttribute {
    Palette = 0x03,
    Priority = 0x20,
    FlipHorizontal = 0x40,
    FlipVertical = 0x80
}

#[allow(dead_code)]
#[derive(Debug, Copy, Clone, PartialEq)]
enum SpritePriority {
    Front,
    Back,
    None
}

#[allow(dead_code)]
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

#[derive(Debug, PartialEq)]
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
            scroll: 0,
            data: 0
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Sprite {
    x: u8,
    y: u8,
    tile_index: u8,
    attributes: u8,
    sprite0: bool
}

impl Default for Sprite {
    fn default() -> Self {
        Sprite {
            x: 0,
            y: 0,
            tile_index: 0,
            attributes: 0,
            sprite0: false,
        }
    }
}

impl Sprite {
    fn get_attribute_value(&self, attr: SpriteAttribute) -> u8 {
        match attr {
            SpriteAttribute::Palette => {
                self.attributes & attr as u8
            },
            _ => self.is_attribute_set(attr) as u8
        }
    }

    fn is_attribute_set(&self, attr: SpriteAttribute) -> bool {
        let attr = attr as u8;
        self.attributes & attr != 0
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
enum PixelMode {
    Background,
    Sprite
}

#[derive(Debug, Copy, Clone)]
struct Pixel {
    r: u8,
    g: u8,
    b: u8,
    a: u8,
    priority: SpritePriority
}

impl Default for Pixel {
    fn default() -> Self {
        Pixel {
            r: 0,
            g: 0,
            b: 0,
            a: 0,
            priority: SpritePriority::None
        }
    }
}

impl Pixel {
    fn new(r: u8, g: u8, b: u8, a: u8, priority: SpritePriority) -> Self {
        Pixel {
            r,
            g,
            b,
            a,
            priority
        }
    }
}

#[derive(Debug)]
struct PixelLines {
    rgba_pixels: [Pixel; PIXEL_X_MAX as usize + 1]
}

impl Default for PixelLines {
    fn default() -> Self {
        PixelLines {
            rgba_pixels: [Pixel::default(); PIXEL_X_MAX as usize + 1]
        }
    }
}

impl PixelLines {

    fn clear(&mut self) {
        self.rgba_pixels = [Pixel::default(); PIXEL_X_MAX as usize+ 1]
    }

    fn get_pixel_rgba(&self, x: u8) -> &Pixel {
        &self.rgba_pixels[x as usize]
    }

    fn set_pixel_rgba(&mut self, x: u8, pixel: Pixel) {
        self.rgba_pixels[x as usize] = pixel;
    }

    fn is_transparent(&self, x: u8) -> bool {
        Palette2C02::is_transparent(self.rgba_pixels[x as usize].a)
    }

    fn merge(&self, other: &PixelLines) -> PixelLines {
        let mut merged_pixels = PixelLines::default();

        for (x, pixel) in self.rgba_pixels.iter().enumerate() {
            merged_pixels.set_pixel_rgba(x as u8, pixel.clone());
        }

        for (x, pixel) in other.rgba_pixels.iter().enumerate() {
            if pixel.priority == SpritePriority::Front || Palette2C02::is_transparent(merged_pixels.get_pixel_rgba(x as u8).a) {
                if Palette2C02::is_transparent(pixel.a) == false {
                    merged_pixels.set_pixel_rgba(x as u8, pixel.clone());
                }
            }
        }

        merged_pixels
    }
}

#[derive(Debug, PartialEq)]
enum PpuState {
    Rendering(u16),
    VBlank(u16),
}

impl Display for PpuState {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            PpuState::Rendering(scanline) => write!(f, "rendering (scanline: {})", scanline),
            PpuState::VBlank(scanline) => write!(f, "vblank (scanline: {})", scanline),
        }
    }
}

pub struct Ppu2c02 {
    register: RefCell<Register>,
    bus: Box<dyn Bus>,
    oam: OAM,
    v: RefCell<u16>,
    t: u16,
    x: u8,
    latch: RefCell<Latch>,
    renderer: RefCell<Renderer>,
    cpu: Rc<RefCell<dyn CPU>>,
    state: PpuState,
    tile_cache: TileCache,
    background_pixels_line: PixelLines,
    sprites_pixels_line: PixelLines
}

#[derive(Debug, Copy, Clone)]
struct Tile {
    #[allow(dead_code)]
    index: u8,
    colors: (u8, u8, u8, u8),
    pattern_table: [u8; MERGED_PATTERN_DATA_SIZE]
}

impl Tile {
    fn new(index: u8, colors: (u8, u8, u8, u8), pattern_table: [u8; MERGED_PATTERN_DATA_SIZE]) -> Self {
        Tile {
            index,
            colors,
            pattern_table
        }
    }
}

impl Default for Tile {
    fn default() -> Self {
        Tile::new(0xFF, (0, 0, 0, 0), [0; MERGED_PATTERN_DATA_SIZE])
    }
}

impl Display for Tile {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        write!(f, "tile - index: 0x{:02X}, colors: 0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X}\n", self.index, self.colors.0, self.colors.1, self.colors.2, self.colors.3)?;
        write!(f, "tile - index: 0x{:02X}, pattern_table:", self.index)?;
        for (index, byte) in self.pattern_table.iter().enumerate() {
            if index % 8 == 0 {
                write!(f, "\n")?;
            }
            write!(f, "{:02X} ", byte)?;
        }
        Ok(())
    }
}

struct TileCache {
    tiles: HashMap<u16, Rc<Tile>>
}

impl Default for TileCache {
    fn default() -> Self {
        TileCache {
            tiles: HashMap::new()
        }
    }
}

impl TileCache {

    fn clear(&mut self) {
        self.tiles.clear();
    }

    fn get_cached_tile(&self, addr: u16) -> Option<Rc<Tile>> {
        if let Some(tile) = self.tiles.get(&addr) {
            Some(tile.clone())
        } else {
            None
        }
    }

    fn set_cached_tile(&mut self, tile: Tile, addr: u16) -> Rc<Tile> {
        self.tiles.insert(addr, Rc::new(tile));
        self.get_cached_tile(addr).unwrap()
    }
}

struct OAM {
    primary: [Sprite; 64],
    secondary: [Sprite; 8],
    sprite_count: usize
}

impl Default for OAM {
    fn default() -> Self {
        OAM {
            primary: [Sprite::default(); 64],
            secondary: [Sprite::default(); 8],
            sprite_count: 0
        }
    }
}

impl OAM {
    fn clear_secondary(&mut self) {
        self.secondary
            .iter_mut()
            .for_each(|s| *s = Sprite::default());

        self.sprite_count = 0;
    }
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
        unreachable!()
    }

    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<u32, PpuError> {
        let mut cycles = start_cycle;
        let cycles_threshold = start_cycle + credits;

        //debug!("PPU: running PPU - cycle: {}, credits: {}, threshold: {}", start_cycle, credits, cycles_threshold);

        loop {
            cycles = cycles + self.render()? as u32;

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
        ////trace!("PPU: registers access: reading byte at 0x{:04X} (0x{:04X})", addr, addr + 0x2000);

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
        //trace!("PPU: registers access: writing byte (0x{:02X}) at 0x{:04X}", value, addr);

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
        unreachable!()
    }

    fn size(&self) -> usize {
        PPU_EXTERNAL_MEMORY_SIZE
    }
}

impl Debug for Ppu2c02 {
    fn fmt(&self, _: &mut Formatter<'_>) -> std::fmt::Result {
        unreachable!()
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
        //trace!("PPU: DMA write to OAM with value 0x{:02X} at OAM addr 0x{:02X}", value, addr);
        self.write_oam_data_register(addr, value);
        Ok(())
    }
}

impl Ppu2c02 {

    fn v_wrapping_add(&self, n: u16) -> u16 {
        self.v.borrow().wrapping_add(n) % (PPU_INTERNAL_ADDRESS_SPACE.1 + 1)
    }

    fn read_control_register(&self) -> u8 {
        self.register.borrow().control
    }

    fn write_control_register(&mut self, value: u8) {
        //trace!("PPU: writing to control register: 0x{:02X}", value);

        self.register.borrow_mut().control = value;
        self.t = (self.t & 0xF3FF) | (((value & 0x03) as u16) << 10);

        if let PpuState::VBlank(_) = self.state {
            if value & 0x80 != 0 && self.get_flag(Status(VBlank)) && self.get_flag(Control(GenerateNmi)) == false {
                //trace!("PPU: forcing NMI as status changed: 0x{:02X}", value);
                let cpu = self.cpu.as_ptr();
                let _ = unsafe { &mut *cpu }.signal_nmi();
            }
        }
    }

    fn read_mask_register(&self) -> u8 {
        self.register.borrow().mask
    }

    fn write_mask_register(&mut self, value: u8) {
        //trace!("PPU: writing to mask register: 0x{:02X}", value);
        self.register.borrow_mut().mask = value;
    }

    fn read_status_register(&self) -> u8 {
        let result = self.register.borrow().status;
        self.set_flag(Status(VBlank), false);
        self.latch.borrow_mut().reset();

        result
    }

    fn write_status_register(&mut self, value: u8) {
        //trace!("PPU: writing to status register: 0x{:02X}", value);
        self.register.borrow_mut().status = value;
    }

    fn read_oam_address_register(&self) -> u8 {
        self.register.borrow().oam_addr
    }

    fn write_oam_address_register(&mut self, value: u8) {
        //trace!("PPU: writing to oam address register: 0x{:02X}", value);
        self.register.borrow_mut().oam_addr = value;
    }

    fn read_oam_data_register(&self, addr: u8) -> u8 {
        let sprite_index = (addr / 4) as usize;
        let offset = addr % 4;

        match offset {
            0 => self.oam.primary[sprite_index].y,
            1 => self.oam.primary[sprite_index].tile_index,
            2 => self.oam.primary[sprite_index].attributes,
            3 => self.oam.primary[sprite_index].x,
            _ => unreachable!(),
        }
    }

    /***
     * OAM addr write
     * https://www.nesdev.org/wiki/PPU_registers#OAMDATA
     ***/
    fn write_oam_data_register(&mut self, addr: u8, value: u8) {
        //trace!("PPU: writing to oam data register: 0x{:02X}: 0x{:02X}", addr, value);

        if let PpuState::Rendering(_) = self.state  {
            //trace!("PPU: ignoring write to OAM address 0x{:02X} as PPU is in state {}", addr, self.state);
            self.register.borrow_mut().oam_addr = addr.wrapping_add(4);
        } else {
            let sprite_index = (addr / 4) as usize;
            let offset = addr % 4;

            match offset {
                0 => self.oam.primary[sprite_index].y = value,
                1 => self.oam.primary[sprite_index].tile_index = value,
                2 => self.oam.primary[sprite_index].attributes = value & !0x1C,
                3 => self.oam.primary[sprite_index].x = value,
                _ => unreachable!(),
            }

            self.register.borrow_mut().oam_addr = addr.wrapping_add(1);
        }
    }

    fn read_scroll_register(&self) -> u8 {
        self.register.borrow().scroll
    }

    fn write_scroll_register(&mut self, value: u8) {
        //trace!("PPU: writing to scroll register: 0x{:02X}", value);

        if self.latch.borrow().state == LatchState::HIGH {
            self.t = (self.t & !0x001F) | ((value as u16) >> 3);
            self.x = value & 0x07;
        } else {
            let a = ((value & 0x07) as u16) << 12;
            let b = ((value >> 3) as u16) << 5;
            self.t = (self.t & !0x73E0) | a | b;
        }

        self.latch.borrow_mut().latch();
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
        //trace!("PPU: writing to PPU addr register: 0x{:02X}", value);

        if self.latch.borrow().state == LatchState::HIGH {
            self.t = (self.t & 0x00FF) | ((value as u16 & 0x3F) << 8);
        } else {
            self.t = (self.t & 0x7F00) | (value as u16);
            *self.v.borrow_mut() = self.t;
        }

        self.latch.borrow_mut().latch();
    }

    /***
     * https://www.nesdev.org/wiki/PPU_registers#PPUDATA
     * https://forums.nesdev.org/viewtopic.php?t=9353
     * read are delayed by 1, however palette read are not
     */
    fn read_data_register(&self) -> Result<u8, MemoryError> {
        let video_addr = *self.v.borrow();
        let incr = self.get_v_increment_value() as u16;
        *self.v.borrow_mut() = self.v_wrapping_add(incr);

        let data = if video_addr >= PALETTE_ADDRESS_SPACE.0 {
            self.bus.read_byte(video_addr)?
        } else {
            let previous_read = self.register.borrow().data;
            self.register.borrow_mut().data = self.bus.read_byte(video_addr)?;
            previous_read
        };

        Ok(data)
    }

    fn write_data_register(&mut self, value: u8) -> Result<(), MemoryError> {
        let incr = self.get_v_increment_value() as u16;
        let incremented_v = self.v_wrapping_add(incr);

        //trace!("PPU: writing to PPU data register: 0x{:02X} (v is: 0x{:04X})", value, *self.v.borrow());
        self.bus.write_byte(*self.v.borrow(), value)?;

        *self.v.borrow_mut() = incremented_v;
        Ok(())
    }

    fn create_mirrored_name_tables_and_connect_to_bus(bus: &mut Box<dyn Bus>, mirroring: PpuNameTableMirroring) -> Result<(), PpuError> {
        let map = match mirroring {
            PpuNameTableMirroring::Vertical => NT_MAP_VERTICAL,
            PpuNameTableMirroring::Horizontal => NT_MAP_HORIZONTAL,
        };

        let mut created_name_tables: Vec<Rc<RefCell<MemoryBank>>> = Vec::new();

        for (i, &(start, end)) in NT_BASES.iter().enumerate() {
            let group = map[i];

            let new_name_table: Rc<RefCell<dyn BusDevice>> = if let Some(name_table) = created_name_tables.get(group) {
                let m = MemoryMirror::new(name_table.clone(), (start, end))?;
                Rc::new(RefCell::new(m))
            } else {
                let m0 = MemoryBank::new(NAME_TABLE_SIZE, (start, end));
                let m1 = Rc::new(RefCell::new(m0));
                created_name_tables.push(m1.clone());
                m1
            };

            new_name_table.borrow_mut().initialize()?;
            bus.add_device(new_name_table)?;
        }

        Ok(())
    }

    pub fn new(chr_rom: Rc<RefCell<dyn BusDevice>>, mirroring: PpuNameTableMirroring, cpu: Rc<RefCell<dyn CPU>>) -> Result<Self, PpuError> {
        let mut bus: Box<dyn Bus> = Box::new(NESBus::new());

        let palette_table = Rc::new(RefCell::new(
            MemoryPalette::new(PALETTE_SIZE, PALETTE_ADDRESS_SPACE)));

        palette_table.borrow_mut().initialize()?;

        bus.add_device(palette_table)?;
        bus.add_device(chr_rom)?;

        Ppu2c02::create_mirrored_name_tables_and_connect_to_bus(&mut bus, mirroring.clone())?;

        let ppu = Ppu2c02 {
            register: RefCell::new(Register::new()),
            bus,
            v: RefCell::new(0),
            t: 0,
            x: 0,
            oam: OAM::default(),
            latch: RefCell::new(Latch::new()),
            renderer: RefCell::new(Renderer::new()),
            cpu,
            state: PpuState::VBlank(261),
            tile_cache: TileCache::default(),
            background_pixels_line: PixelLines::default(),
            sprites_pixels_line: PixelLines::default(),
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

    fn fetch_palette(&self, tile_x: u8, tile_y: u8, attribute_table_addr: u16) -> Result<u8, PpuError> {
        let block_x = tile_x / 4;
        let block_y = tile_y / 4;
        let attribute_table_address = attribute_table_addr + (block_y as u16 * 8) + block_x as u16;
        let attribute_data = self.bus.read_byte(attribute_table_address)?;

        let quadrant_x = (tile_x % 4) / 2;
        let quadrant_y = (tile_y % 4) / 2;
        let shift = 2 * (quadrant_y * 2 + quadrant_x);
        let palette = (attribute_data >> shift ) & 0x03;

        //trace!("PPU: attribute_table_address: 0x{:04X}, palette: 0x{:02X}", attribute_table_address, palette);
        Ok(palette)
    }

    fn flip_horizontal(&self, data_plane0: &mut u8, data_plane1: &mut u8)  {
        *data_plane0 = data_plane0.reverse_bits();
        *data_plane1 = data_plane1.reverse_bits();
    }

    fn merge_bit_planes(&self, data_plane0: &mut u8, data_plane1: &mut u8) -> Vec<u8> {
        let mut line_pattern_data = vec![0u8; 8];

        for bit in 0..=7 {
            let value0 = (*data_plane0 >> (7 - bit)) & 0x01;
            let value1 = (*data_plane1 >> (7 - bit)) & 0x01;
            let combined = (value1 << 1) | value0;
            line_pattern_data[bit] = combined;
        }

        line_pattern_data
    }

    fn fetch_pattern_data(&self, tile_index: u8, pattern_table_addr: u16, flip_horizontal: bool) -> Result<Vec<u8>, PpuError> {
        let mut pattern_data= vec![];

        for line in 0..=7 {
            let mut pattern_data0 = self.bus.read_byte(pattern_table_addr + (tile_index as u16 * PATTERN_DATA_SIZE as u16) + line as u16)?;
            let mut pattern_data1 = self.bus.read_byte(pattern_table_addr + (tile_index as u16 * PATTERN_DATA_SIZE as u16) + line as u16 + (PATTERN_DATA_SIZE as u16 / 2))?;

            if flip_horizontal {
                self.flip_horizontal(&mut pattern_data0, &mut pattern_data1);
            }

            let mut line_pattern_data = self.merge_bit_planes(&mut pattern_data0, &mut pattern_data1);
            pattern_data.append(&mut line_pattern_data);
        }

        //trace!("PPU: pattern_data: {:?}", pattern_data);
        Ok(pattern_data)
    }

    fn fetch_line_pattern_data(&self, tile: &Tile, line: u8, offset_x: u8, size: usize) -> Vec<u8> {
        let a = (line * 8) as usize + offset_x as usize;
        let b = a + size;

        tile.pattern_table[a..b].to_vec()
    }

    fn fetch_tile_index(&self, tile_x: u8, tile_y: u8, base_name_table_addr: u16) -> Result<u8, PpuError> {
        let name_table_index = tile_x as u16 + (tile_y as u16 * 32);
        let addr = base_name_table_addr + name_table_index;
        let tile_index = self.bus.read_byte(addr)?;

        //trace!("PPU: tile_index at 0x{:04X} in name table: 0x{:02X}",addr, tile_index);
        Ok(tile_index)
    }

    fn get_background_palette_address(&self, palette: u8) -> u16 {
        let palette_address = PALETTE_ADDRESS_SPACE.0 + (palette as u16 * 4);

        //trace!("PPU: background palette address: 0x{:04X}", palette_address);
        palette_address
    }

    fn get_sprite_palette_address(&self, palette: u8) -> u16 {
        let palette_address = SPRITE_PALETTE_ADDR + (palette as u16 * 4);

        //trace!("PPU: sprite palette address: 0x{:04X}", palette_address);
        palette_address
    }

    fn get_palette_colors(&self, palette_addr: u16) -> Result<(u8, u8, u8, u8), PpuError> {
        let mut colors = [0u8; 4];

        colors[0] = self.bus.read_byte(PALETTE_ADDRESS_SPACE.0)?;

        for i in 1..=3 {
            colors[i] = self.bus.read_byte(palette_addr + i as u16)?;
        }

        Ok((colors[0], colors[1], colors[2], colors[3]))
    }

    fn get_background_palette_colors(&self, palette: u8) -> Result<(u8, u8, u8, u8), PpuError> {
        let palette_address = self.get_background_palette_address(palette);
        let colors = self.get_palette_colors(palette_address)?;

        //trace!("PPU: background palette color: (0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X})", colors.0, colors.1, colors.2, colors.3);
        Ok(colors)
    }

    fn get_sprite_palette_colors(&self, palette: u8) -> Result<(u8, u8, u8, u8), PpuError> {
        let palette_address = self.get_sprite_palette_address(palette);
        let colors = self.get_palette_colors(palette_address)?;

        //trace!("PPU: sprite palette color: (0x{:02X}, 0x{:02X}, 0x{:02X}, 0x{:02X})", colors.0, colors.1, colors.2, colors.3);
        Ok(colors)
    }

    fn detect_sprite_0_hit_and_set_status_flag(&self, pixel_pos_x: u8) {
        let background_transparency = self.background_pixels_line.is_transparent(pixel_pos_x);
        let sprite_transparency = self.sprites_pixels_line.is_transparent(pixel_pos_x);

        if sprite_transparency == false && background_transparency == false {
            self.set_flag(Status(Sprite0Hit), true);
        }
    }

    fn set_pixel(&mut self, pixel_pos_x: u8, _: u8, line_pattern_data: &[u8],
                 palette: (u8, u8, u8, u8), mode: PixelMode, priority: SpritePriority, sprite0_hit_detect: bool) {

        line_pattern_data.iter().enumerate().for_each(|(pixel_num, color)| {
            //trace!("PPU: x: {}, y: {}, color: {}, mode: {:?}, palette: {:?}", pixel_pos_x, pixel_pos_y, color, mode, palette);

            let (r, g, b, a) = match (color, mode) {
                (0, PixelMode::Background) => Palette2C02::rgba_transparent(palette.0),
                (0, PixelMode::Sprite) => Palette2C02::rgba_transparent(palette.0),
                (1, _) => Palette2C02::rgba_opaque(palette.1),
                (2, _) => Palette2C02::rgba_opaque(palette.2),
                (3, _) => Palette2C02::rgba_opaque(palette.3),
                _ => unreachable!("unknown color: {}", color)
            };

            let pixel_pos_x_plus_pixel = pixel_pos_x + pixel_num as u8;
            let pixel = Pixel::new(r, g, b, a, priority);

            match mode {
                PixelMode::Background => {
                    self.background_pixels_line.set_pixel_rgba(pixel_pos_x_plus_pixel, pixel)
                },

                PixelMode::Sprite => {
                    if Palette2C02::is_transparent(pixel.a) == false {
                        self.sprites_pixels_line.set_pixel_rgba(pixel_pos_x_plus_pixel, pixel);
                    }

                    if sprite0_hit_detect {
                        self.detect_sprite_0_hit_and_set_status_flag(pixel_pos_x_plus_pixel);
                    }
                },
            }
        });
    }

    fn get_background_pattern_table_addr(&self) -> u16 {
        let pattern_table_addr = if self.get_flag(Control(BackgroundPatternTableAddr)) {
            PATTERN_TABLE_RIGHT_ADDR
        } else {
            PATTERN_TABLE_LEFT_ADDR
        };

        //trace!("PPU: background pattern table address: 0x{:04X}", pattern_table_addr);
        pattern_table_addr
    }

    fn get_sprites_pattern_table_addr(&self) -> u16 {
        let pattern_table_addr = if self.get_flag(Control(SpritePatternTableAddr)) {
            PATTERN_TABLE_RIGHT_ADDR
        } else {
            PATTERN_TABLE_LEFT_ADDR
        };

        //trace!("PPU: sprite pattern table address: 0x{:04X}", pattern_table_addr);
        pattern_table_addr
    }

    fn get_attribute_table_addr(&self, base_name_table_addr: u16) -> u16 {
        let attribute_table_addr= base_name_table_addr + (NAME_TABLE_SIZE - ATTRIBUTE_TABLE_SIZE) as u16;

        //trace!("PPU: attribute table: 0x{:04X}", attribute_table_addr);
        attribute_table_addr
    }

    fn get_name_table_addr(&self, select: u8) -> u16 {
        NT_BASES[select as usize].0
    }

    fn get_name_table_addr_from_v(&self) -> u16 {
        let select = ((*self.v.borrow() >> 10) & 0x03) as u8;
        let base_name_table_addr = self.get_name_table_addr(select);

        //trace!("PPU: base name table from control register: 0x{:04X}", base_name_table_addr);
        base_name_table_addr
    }



    fn fetch_tile(&self, coarse_x: u8, coarse_y: u8, name_table_addr: u16, pattern_table_addr: u16, attribute_table_addr: u16) -> Result<Tile, PpuError> {
        let tile_index = self.fetch_tile_index(coarse_x, coarse_y, name_table_addr)?;
        let palette = self.fetch_palette(coarse_x, coarse_y, attribute_table_addr)?;
        let colors = self.get_background_palette_colors(palette)?;
        let pattern_data = self.fetch_pattern_data(tile_index, pattern_table_addr, false)?;

        let tile = Tile::new(tile_index, colors, vec_to_array::<64>(pattern_data));

        //trace!("{}", tile);
        Ok(tile)
    }

    fn get_tile(&mut self, coarse_x: u8, coarse_y: u8, name_table_addr: u16, pattern_table_addr: u16, attribute_table_addr: u16) -> Result<Rc<Tile>, PpuError> {
        let addr = name_table_addr | (coarse_y as u16) << 5  | coarse_x as u16;

        let tile = if let Some(cached_tile) = self.tile_cache.get_cached_tile(addr) {
            //trace!("cache hit: coarse_x: {}, coarse_y: {}, tile: 0x{:02X}", coarse_x, coarse_y, cached_tile.index);
            cached_tile
        } else {
            //trace!("cache miss: coarse_x: {}, coarse_y: {}", coarse_x, coarse_y);
            let fetched_tile = self.fetch_tile(coarse_x, coarse_y, name_table_addr, pattern_table_addr, attribute_table_addr)?;

            let cache = &mut self.tile_cache;
            let cached_tile = cache.set_cached_tile(fetched_tile, addr);

            cached_tile.clone()
        };

        Ok(tile)
    }

    /***
     * coase_x: ...ABCDE <- v: ........ ...ABCDE
     ***/
    fn get_coarse_x(&self) -> u8 {
        (*self.v.borrow() & 0x1F) as u8
    }

    /***
     * coarse_y: ...ABCDE <- v: .....AB CDE.....
     ***/
    fn get_coarse_y(&self) -> u8 {
        ((*self.v.borrow() & 0x3E0) >> 5) as u8
    }

    /***
     * fine_y: .....ABC <- v: ABC.... ........
     ***/
    fn get_fine_y(&self) -> u8 {
        ((*self.v.borrow() & 0x7000) >> 12) as u8
    }

    fn get_fine_x(&self) -> u8 {
        self.x
    }

    /***
     * v: ....... ...BCDEF <- x: .BCDEF
     * v: ....A.. ...BCDEF <- nametable: .....A.. ........
     */
    fn update_v_coarse_x(&mut self, nametable: u16, coarse_x: u8) {
        let mut v = *self.v.borrow_mut();

        v = (v & 0xFFE0) | (coarse_x as u16 & 0x1F);
        v = (v & 0xFBFF) | (nametable & 0x400);

        *self.v.borrow_mut() = v;
    }


    /***
     * v: .....BC DEF..... <- y: .BCDEF
     * v: ...A... ........ <- nametable: ....A... ........
     */
    fn update_v_fine_and_coarse_y(&mut self, nametable: u16, fine_y: u8, coarse_y: u8) {
        let mut v = *self.v.borrow_mut();

        v = (v & 0xFC1F) | (((coarse_y & 0x1F) as u16) << 5);
        v = (v & 0xF7FF) | (nametable & 0x0800);

        v = (v & !0x7000) | ((fine_y as u16) << 12);

        *self.v.borrow_mut() = v;
    }

    /***
     * v: ....A.. ...BCDEF <- t: ....A.. ...BCDEF
     */
    fn put_horizontal_t_into_v(&mut self) {
        let mut v = *self.v.borrow_mut();

        v = (v & !0x041F) | (self.t & 0x041F);
        *self.v.borrow_mut() = v;
    }

    /***
     * v: GHIA.BC DEF..... <- t: GHIA.BC DEF.....
     */
    fn put_vertical_t_into_v(&mut self) {
        let mut v = *self.v.borrow_mut();

        v = (v & !0x7BE0) | (self.t & 0x7BE0);
        *self.v.borrow_mut() = v;
    }

    fn coarse_x_increment(&self, name_table_addr: u16, coarse_x: u8) -> (u16, u8) {
        if coarse_x == 31 {
            let addr = name_table_addr ^ 0x0400;
            (addr, 0)
        } else {
            (name_table_addr, coarse_x + 1)
        }
    }

    fn fine_and_coarse_y_increment(&self, name_table_addr: u16, fine_y: u8, coarse_y: u8) -> (u16, u8, u8) {
        if fine_y < 7 {
            (name_table_addr, fine_y + 1, coarse_y)
        } else if coarse_y == 29 {
                let addr = name_table_addr ^ 0x0800;
                (addr, 0, 0)
        } else if coarse_y == 31 {
                (name_table_addr, 0, 0)
        } else {
                (name_table_addr, 0, coarse_y + 1)
        }
    }

    /***
     *
     * v is:
     * yyyNNYYYYYXXXXX
     *
     * y, the fine Y position, holding the Y position within a 8x8-pixel tile.
     * N, the index for choosing the name table.
     * Y, the 5-bit coarse Y position, which can reference one of the 30 8x8 tiles on the screen in the vertical direction.
     * X, the 5-bit coarse X position, which can reference one of the 32 8x8 tiles on the screen in the horizontal direction.
     *
     ***/
    fn render_background(&mut self, scanline: u16) -> Result<(), PpuError> {
        let name_table_addr_from_v = self.get_name_table_addr_from_v();
        let mut name_table_addr = name_table_addr_from_v;

        let pattern_table_addr = self.get_background_pattern_table_addr();
        let mut attribute_table_addr = self.get_attribute_table_addr(name_table_addr);

        let mut fine_y = self.get_fine_y();
        let mut coarse_y = self.get_coarse_y();

        let mut coarse_x = self.get_coarse_x();
        let pixel_pos_y = scanline;

        let mut fine_x = self.get_fine_x();

        //trace!("PPU: rendering background, scanline: {}, nametable: 0x{:04X}, fine_y: {}, coarse_y: {}, coarse_x: {}, pixel_pos_y: {}",
        //    scanline, name_table_addr, fine_y, coarse_y, coarse_x, pixel_pos_y);

        self.background_pixels_line.clear();

        let mut pixel_pos_x= 0u8;
        loop {
            let tile =  self.get_tile(coarse_x, coarse_y, name_table_addr, pattern_table_addr, attribute_table_addr)?;

            let size = if PIXEL_X_MAX - pixel_pos_x >= 8 { 8usize - fine_x as usize } else { (PIXEL_X_MAX - pixel_pos_x) as usize + 1 };
            let line_pattern_data = self.fetch_line_pattern_data(tile.as_ref(), fine_y, fine_x, size);
            let palette = tile.colors;

            self.set_pixel(pixel_pos_x, pixel_pos_y as u8, &line_pattern_data, palette,
                           PixelMode::Background, SpritePriority::None, false);

            if pixel_pos_x + (size as u8 - 1) == PIXEL_X_MAX {
                break;
            } else {
                pixel_pos_x += size as u8;

                if fine_x != 0 {
                    fine_x = 0;
                }

                (name_table_addr, coarse_x) = self.coarse_x_increment(name_table_addr, coarse_x);
                attribute_table_addr = self.get_attribute_table_addr(name_table_addr);
            }
        }

        name_table_addr = self.get_name_table_addr_from_v();
        (name_table_addr, fine_y, coarse_y) = self.fine_and_coarse_y_increment(name_table_addr, fine_y, coarse_y);

        self.update_v_coarse_x(name_table_addr, coarse_x);
        self.update_v_fine_and_coarse_y(name_table_addr, fine_y, coarse_y);

        //trace!("PPU: rendered background for scanline: {}", scanline);
        Ok(())
    }

    fn is_scanline_in_sprite_range(&self, scanline: u16, sprite: &Sprite, size: u8) -> bool {
        let sprite_y_min = sprite.y as u16;
        let sprite_y_max = (sprite_y_min + size as u16).clamp(0, PIXEL_Y_MAX as u16);

        scanline >= sprite_y_min && scanline < sprite_y_max
    }

    fn get_flip_values(&self, sprite: &Sprite) -> (bool, bool) {
        (sprite.get_attribute_value(FlipHorizontal) != 0,
         sprite.get_attribute_value(FlipVertical) != 0)
    }

    fn get_tile_by_sprite_definition(&self, sprite: &Sprite, is_sprite_8x16: bool, line: u8, pattern_table_addr: u16) -> Result<(Tile, u8), PpuError> {
        let palette = sprite.get_attribute_value(SpriteAttribute::Palette);
        let colors = self.get_sprite_palette_colors(palette)?;

        let (flip_horizontal, flip_vertical) = self.get_flip_values(&sprite);

        let (tile_index, fixed_pattern_table_addr, tile_offset) = if is_sprite_8x16 {
            // ignore pattern table from control register and use LSB of sprite index for pattern table
            let fixed_pattern_table_addr = if (sprite.tile_index & 1) == 0 {
                PATTERN_TABLE_LEFT_ADDR
            } else {
                PATTERN_TABLE_RIGHT_ADDR
            };

            // apply vertical flip to the 0..15 row within the sprite
            let row = if flip_vertical { 15 - line } else { line };

            // choose half and fine row 0..7
            let pick_top_tile = row < 8;
            let tile_index = if pick_top_tile { sprite.tile_index & 0xFE } else { sprite.tile_index | 1 };
            let tile_offset = row & 7;

            // fetch pattern data and create a tile, force flip vertical to false as it was already flipped
            (tile_index, fixed_pattern_table_addr, tile_offset)
        } else {
            let tile_offset = if flip_vertical { 7 - (line & 7) } else { line & 7 };
            (sprite.tile_index, pattern_table_addr, tile_offset)
        };

        let pattern_data = self.fetch_pattern_data(tile_index, fixed_pattern_table_addr, flip_horizontal)?;
        let tile = Tile::new(tile_index, colors, vec_to_array::<64>(pattern_data));

        Ok((tile, tile_offset))
    }

    fn do_sprite_evaluation(&mut self, scanline: u16) -> Result<(), PpuError> {
        self.oam.clear_secondary();
        let sprite_size = if self.get_flag(Control(SpriteSize)) { 16u8 } else { 8u8 };

        for i in 0..self.oam.primary.len() {
            let sprite = &self.oam.primary[i];

            if self.is_scanline_in_sprite_range(scanline, sprite, sprite_size) {
                //trace!("sprite: {:?}", sprite);
                self.oam.secondary[self.oam.sprite_count] = sprite.clone();

                if i == 0 {
                    self.oam.secondary[self.oam.sprite_count].sprite0 = true;
                }

                self.oam.sprite_count += 1;

                if self.oam.sprite_count == self.oam.secondary.len() {
                    break
                }
            }
        }

        //trace!("evaluated {} sprites for next scanline ({} -> {})", self.oam.sprite_count, scanline, scanline + 1);
        Ok(())
    }

    fn get_sprite_priority(&self, sprite: &Sprite) -> SpritePriority {
        if sprite.get_attribute_value(SpriteAttribute::Priority) == 1 {
            SpritePriority::Back
        } else {
            SpritePriority::Front
        }
    }

    fn detect_sprite_0_hit(&self, is_sprite_0: bool) -> bool {
        if self.get_flag(Mask(ShowBackground)) == true && self.get_flag(Status(Sprite0Hit)) == false && is_sprite_0 {
            true
        } else {
            false
        }
    }

    /***
     * [...] all sprites are displayed one pixel lower than their Y coordinate says [...]
     * https://www.reddit.com/r/EmuDev/comments/x1ol0k/nes_emulator_working_perfectly_except_one/
     */
    fn render_sprites(&mut self, scanline: u16) -> Result<(), PpuError> {
        let is_sprite_8x16  = self.get_flag(Control(SpriteSize));
        let sprite_pattern_table_addr = self.get_sprites_pattern_table_addr();

        //let sprite_size = if self.get_flag(Control(SpriteSize)) { 16u8 } else { 8u8 };
        //trace!("rendering {} sprites for scanline: {}", self.oam.sprite_count, scanline);

        self.sprites_pixels_line.clear();

        for i in (0..self.oam.sprite_count).rev() {
            let sprite = &self.oam.secondary[i];
            let pixel_pos_y = scanline as u8 - (sprite.y + 1);
            let width = if PIXEL_X_MAX - sprite.x > SPRITE_WIDTH { SPRITE_WIDTH as usize } else { (PIXEL_X_MAX - sprite.x) as usize };

            let (tile, tile_offset) = self.get_tile_by_sprite_definition(sprite, is_sprite_8x16, pixel_pos_y, sprite_pattern_table_addr)?;

            let sprite0_hit_detect = self.detect_sprite_0_hit(sprite.sprite0);
            let priority = self.get_sprite_priority(sprite);

            let line_pattern_data = self.fetch_line_pattern_data(&tile, tile_offset, 0, width);
            let palette = tile.colors;

            self.set_pixel(sprite.x, scanline as u8, &line_pattern_data, palette, PixelMode::Sprite, priority, sprite0_hit_detect);
        }

        Ok(())
    }

    fn write_pixels_lines_to_frame(&self, scanline: u16, show_background: bool, show_sprites: bool) {
        let pixels = match (show_background, show_sprites) {
            (true, true) => &self.background_pixels_line.merge(&self.sprites_pixels_line),
            (true, false) => &self.background_pixels_line,
            (false, true) => &self.sprites_pixels_line,
            (false, false) => return,
        };

        pixels.rgba_pixels.iter().enumerate().for_each(|(x, pixel)| {
            self.renderer.borrow_mut().frame_as_mut().set_pixel(x as u8, scanline as u8, (pixel.r, pixel.g, pixel.b));
        });
    }

    fn render_scanline(&mut self) -> Result<(), PpuError> {
        //trace!("PPU: scanline starting: {}", self.state);

        /***
         *  At dot 257 of each scanline:
         *  If rendering is enabled, the PPU copies all bits related to horizontal position from t to v:
         *
         *  v: ....A.. ...BCDEF <- t: ....A.. ...BCDEF
         *
         *  During dots 280 to 304 of the pre-render scanline (end of vblank)
         *  If rendering is enabled, at the end of vblank,
         *  shortly after the horizontal bits are copied from t to v at dot 257,
         *  the PPU will repeatedly copy the vertical bits from t to v from dots 280 to 304,
         *  completing the full initialization of v from t:
         *
         *  v: GHIA.BC DEF..... <- t: GHIA.BC DEF.....
         *
         ***/
        match self.state {
            PpuState::VBlank(261) => {
                self.set_flag(Status(VBlank), false);
                self.set_flag(Status(Sprite0Hit), false);
                self.set_flag(Status(SpriteOverflow), false);

                self.register.borrow_mut().oam_addr = 0;
                self.state = PpuState::Rendering(0);

                if self.get_flag(Mask(ShowBackground)) || self.get_flag(Mask(ShowSprites)) {
                    self.put_horizontal_t_into_v();
                    self.put_vertical_t_into_v();
                }
            },

            PpuState::Rendering(scanline) if scanline <= 239 => {
                let show_background = self.get_flag(Mask(ShowBackground));
                let show_sprites = self.get_flag(Mask(ShowSprites));

                if show_background {
                    self.render_background(scanline)?;
                    self.put_horizontal_t_into_v();
                }

                if show_sprites {
                    self.render_sprites(scanline)?;
                }

                if show_background || show_sprites {
                    self.do_sprite_evaluation(scanline)?;
                } else {
                    self.oam.clear_secondary();
                }

                self.write_pixels_lines_to_frame(scanline, show_background, show_sprites);

                self.register.borrow_mut().oam_addr = 0;
                self.state = PpuState::Rendering(scanline + 1);
            },

            PpuState::Rendering(240) => {
                self.renderer.borrow_mut().update();
                self.tile_cache.clear();
                self.state = PpuState::Rendering(241);
            },

            PpuState::Rendering(241) => {
                self.set_flag(Status(VBlank), true);
                self.state = PpuState::VBlank(242);

                if self.get_flag(Control(GenerateNmi)) {
                    self.cpu.borrow_mut().signal_nmi()?;
                }
            },

            PpuState::VBlank(scanline) if scanline >= 242 && scanline <= 260 => {
                self.state = PpuState::VBlank(scanline + 1);
            },

            _ => unreachable!("render_scanline()")
        }

        //trace!("PPU: scanline ending: {}", self.state);
        Ok(())
    }

    fn render(&mut self) -> Result<u16, PpuError> {

        let (_, _): (Result<(), PpuError>, Duration) = measure_exec_time(|| {
            self.render_scanline()?;
            Ok(())
        });

        //trace!("PPU: rendered line in: {} ms", duration.as_millis());
        Ok(CLOCK_CYCLES_PER_SCANLINE)
    }
}