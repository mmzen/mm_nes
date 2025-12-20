// Authorship: Human 60% | Claude 40%
use std::cell::{Cell, RefCell};
use std::fmt::{Debug, Display, Formatter};
use std::rc::Rc;
use log::info;
use crate::bus::Bus;
use crate::bus_device::{BusDevice, BusDeviceType};
use crate::config_spec::{ConfigSpec, Configurable};
use crate::cpu::CPU;
use crate::dma_device::DmaDevice;
use crate::nes_frame::NesFrame;
use crate::memory::{Memory, MemoryError};
use crate::memory_ciram::{CiramMemory, PpuNameTableMirroring};
use crate::memory_palette::MemoryPalette;
use crate::nes_bus::NESBus;
use crate::palette::Palette;
use crate::palette_2c02::Palette2C02;
use crate::ppu::{PPU, PpuError, PpuType};
use crate::ppu_2c02::ControlFlag::{BackgroundPatternTableAddr, GenerateNmi, SpritePatternTableAddr, SpriteSize, VramIncrement};
use crate::ppu_2c02::MaskFlag::{ShowBackground, ShowSprites};
use crate::ppu_2c02::PpuFlag::{Control, Mask, Status};
use crate::ppu_2c02::SpriteAttribute::{FlipHorizontal, FlipVertical};
use crate::ppu_2c02::StatusFlag::{Sprite0Hit, SpriteOverflow, VBlank};
use crate::renderer::Renderer;
use crate::util::vec_to_array;

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
const SPRITE_WIDTH: u8 = 8;
const PATTERN_DATA_SIZE: usize = 16;
const MERGED_PATTERN_DATA_SIZE: usize = 64;

const PPU_DOTS_PER_SCANLINE: u32 = 341;
const FIXED_POINT_SHIFT: u32 = 16;
const FIXED_POINT_ONE: u64 = 1u64 << FIXED_POINT_SHIFT;

// Dot-level timing constants
const DOTS_PER_SCANLINE: u16 = 341;
#[allow(dead_code)]
const VISIBLE_DOTS: u16 = 256;  // Will be used for sprite 0 hit detection in Phase 3
const VBLANK_SET_DOT: u16 = 1;  // VBlank flag set at dot 1 of NMI scanline

// PPU open bus decay constant
// Each bit on the PPU data bus decays independently after ~600ms
// NTSC PPU: 5.369 MHz = ~5.37M dots/sec, so 600ms ≈ 3.22M dots
// Using a slightly lower value to ensure decay happens "before 1 second"
const OPEN_BUS_DECAY_DOTS: u64 = 3_000_000;

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

/// Public enum for test access to PPU flags (Phase 5.4: mid-scanline effects)
#[cfg(test)]
#[derive(Debug, Clone, Copy)]
pub enum PpuFlagType {
    ShowBackground,
    ShowSprites,
    VBlank,
    Sprite0Hit,
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
    background_pixels_line: PixelLines,
    sprites_pixels_line: PixelLines,
    config: ConfigSpec,
    cycles_per_scanline_fp: u64,
    cycles_acc_fp: u64,
    nmi_suppressed: Cell<bool>,
    // PPU open bus - tracks last value on the internal data bus
    open_bus: Cell<u8>,
    // Track total PPU dots for open bus decay calculation
    total_dots: Cell<u64>,
    // Track when each bit was last refreshed (for decay calculation)
    open_bus_refresh_dots: [Cell<u64>; 8],
    // Dot-level timing state
    current_dot: u16,           // 0-340: current dot position within scanline
    current_scanline: u16,      // 0-261 (NTSC): current scanline
    scanline_rendered: bool,    // true if current scanline's pixels have been rendered
    // Sprite 0 hit tracking for dot-accurate detection
    sprite0_hit_pending: bool,  // true if sprite 0 hit detected during rendering
    sprite0_hit_x: u16,         // X position (dot) where sprite 0 hit should be triggered
    // Partial scanline rendering support
    last_rendered_dot: u16,     // Last dot that was rendered (0-255 for visible, 256+ for hblank)

    // =========================================================================
    // Background shift registers for dot-level rendering (Phase 5)
    // =========================================================================
    // Two 16-bit shift registers hold pattern table data for 2 tiles (current + next)
    // The upper 8 bits hold the next tile, lower 8 bits hold the current tile
    bg_shift_pattern_lo: u16,   // Pattern table low plane (bit 0 of color)
    bg_shift_pattern_hi: u16,   // Pattern table high plane (bit 1 of color)

    // Two 8-bit shift registers hold palette attributes for 2 tiles
    // These contain the same value for all 8 pixels of a tile
    bg_shift_attrib_lo: u16,    // Attribute bit 0 (bit 2 of color)
    bg_shift_attrib_hi: u16,    // Attribute bit 1 (bit 3 of color)

    // Latches for next tile data (loaded during fetch cycles, transferred to shift registers)
    bg_next_tile_id: u8,        // Nametable byte (tile ID)
    bg_next_tile_attrib: u8,    // Attribute byte (2-bit palette selection)
    bg_next_tile_lo: u8,        // Pattern table low byte
    bg_next_tile_hi: u8,        // Pattern table high byte

    // Dot-level rendering mode flag
    dot_level_rendering: bool,  // true = per-dot rendering, false = scanline rendering (fallback)
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

impl Configurable for Ppu2c02 {
    fn set_config(&mut self, config: ConfigSpec) {
        self.config = config;
        self.recompute_timing();
        self.state = PpuState::VBlank(self.pre_render_scanline());
        // Sync dot-level timing with state
        self.current_scanline = self.pre_render_scanline();
        self.current_dot = 0;
        self.scanline_rendered = false;
        // Reset sprite 0 hit tracking
        self.sprite0_hit_pending = false;
        self.sprite0_hit_x = 0;
        // Reset partial scanline rendering
        self.last_rendered_dot = 0;
        // Reset open bus and decay tracking
        self.open_bus.set(0);
        self.total_dots.set(0);
        for bit in 0..8 {
            self.open_bus_refresh_dots[bit].set(0);
        }
    }
}

impl PPU for Ppu2c02 {
    fn reset(&mut self) -> Result<(), PpuError> {
        info!("resetting PPU");

        self.register.borrow_mut().control = 0;
        self.register.borrow_mut().mask = 0;
        self.register.borrow_mut().status = 0;
        self.register.borrow_mut().oam_addr = 0;
        self.register.borrow_mut().scroll = 0;
        self.register.borrow_mut().data = 0;

        *self.v.borrow_mut() = 0;
        self.t = 0;
        self.x = 0;
        self.oam = OAM::default();
        self.latch.borrow_mut().reset();
        self.state = PpuState::VBlank(0);
        self.background_pixels_line = PixelLines::default();
        self.sprites_pixels_line = PixelLines::default();
        self.renderer = RefCell::new(Renderer::new());
        self.nmi_suppressed.set(false);

        // Reset dot-level timing to pre-render scanline
        self.current_dot = 0;
        self.current_scanline = self.pre_render_scanline();
        self.scanline_rendered = false;
        // Reset sprite 0 hit tracking
        self.sprite0_hit_pending = false;
        self.sprite0_hit_x = 0;
        // Reset partial scanline rendering
        self.last_rendered_dot = 0;
        // Reset open bus and decay tracking
        self.open_bus.set(0);
        self.total_dots.set(0);
        for bit in 0..8 {
            self.open_bus_refresh_dots[bit].set(0);
        }

        self.set_flag(Status(VBlank), true);

        self.recompute_timing();

        Ok(())
    }

    fn panic(&self, _: &PpuError) {
        unreachable!()
    }

    /// Run the PPU for the given number of CPU cycles (credits).
    /// Converts CPU cycles to PPU dots and advances the PPU accordingly.
    /// Returns the new cycle count and an optional completed frame.
    fn run(&mut self, start_cycle: u32, credits: u32) -> Result<(u32, Option<NesFrame>), PpuError> {
        // Convert CPU cycles to PPU dots
        // NTSC: 3 PPU dots per CPU cycle, PAL: ~3.2 dots per CPU cycle
        let dots_per_cpu_cycle = self.config.ppu_clock_hz / self.config.cpu_clock_hz;
        let ppu_dots = ((credits as f64) * dots_per_cpu_cycle).round() as u32;

        // Advance PPU by the calculated dots
        let frame = self.advance_dots_internal(ppu_dots)?;

        Ok((start_cycle + credits, frame))
    }

    fn frame(&self) -> NesFrame {
        self.renderer.borrow().frame().clone()
    }

    fn advance_dots(&mut self, dots: u32) -> Result<Option<NesFrame>, PpuError> {
        self.advance_dots_internal(dots)
    }

    fn get_dot(&self) -> u16 {
        self.current_dot
    }

    fn get_scanline(&self) -> u16 {
        self.current_scanline
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

        // Refresh open bus bits based on which register was read
        // Each register that returns valid data refreshes all 8 bits
        // Reading $2002 refreshes bits 7-5 with status, bits 4-0 are NOT refreshed (they read decayed open bus)
        match addr {
            0x02 => {
                // $2002: Only bits 7,6,5 are driven by PPU status
                self.refresh_open_bus_bits(0xE0, value);
            }
            _ => {
                // All other registers refresh all 8 bits
                self.refresh_open_bus_bits(0xFF, value);
            }
        }

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

        // Update open bus on every write to PPU registers - refresh all 8 bits
        self.refresh_open_bus_bits(0xFF, value);

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

    fn get_virtual_address_range(&self) -> (u16, u16) {
        PPU_EXTERNAL_ADDRESS_SPACE
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
        // $2000 is write-only, returns decayed open bus
        self.get_decayed_open_bus()
    }

    fn write_control_register(&mut self, value: u8) {
        let old_nmi_enable = self.get_flag(Control(GenerateNmi));

        self.register.borrow_mut().control = value;
        self.t = (self.t & 0xF3FF) | (((value & 0x03) as u16) << 10);

        let new_nmi_enable = (value & 0x80) != 0;
        
        if let PpuState::VBlank(_) = self.state {
            if self.get_flag(Status(VBlank)) {
                if !old_nmi_enable && new_nmi_enable {
                    let cpu = self.cpu.as_ptr();
                    let _ = unsafe { &mut *cpu }.signal_nmi();
                } else if old_nmi_enable && !new_nmi_enable {
                    let cpu = self.cpu.as_ptr();
                    let _ = unsafe { &mut *cpu }.clear_nmi();
                }
            }
        }
    }

    fn read_mask_register(&self) -> u8 {
        // $2001 is write-only, returns decayed open bus
        self.get_decayed_open_bus()
    }

    fn write_mask_register(&mut self, value: u8) {
        //trace!("PPU: writing to mask register: 0x{:02X}", value);
        self.register.borrow_mut().mask = value;
    }

    fn read_status_register(&self) -> u8 {
        let status = self.register.borrow().status;
        let decayed_open_bus = self.get_decayed_open_bus();

        if let PpuState::Rendering(scanline) = self.state {
            if scanline == self.config.nmi_scanline && !self.get_flag(Status(VBlank)) {
                self.nmi_suppressed.set(true);
            }
        }

        self.set_flag(Status(VBlank), false);
        self.latch.borrow_mut().reset();

        // $2002: bits 7,6,5 are status, bits 4-0 are decayed open bus
        (status & 0xE0) | (decayed_open_bus & 0x1F)
    }

    fn write_status_register(&mut self, value: u8) {
        //trace!("PPU: writing to status register: 0x{:02X}", value);
        self.register.borrow_mut().status = value;
    }

    fn read_oam_address_register(&self) -> u8 {
        // $2003 is write-only, returns decayed open bus
        self.get_decayed_open_bus()
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
        // $2005 is write-only, returns decayed open bus
        self.get_decayed_open_bus()
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
        // $2006 is write-only, returns open bus
        self.get_decayed_open_bus()
    }

    /// Refresh specific bits of the open bus (set their refresh time to now)
    fn refresh_open_bus_bits(&self, mask: u8, value: u8) {
        let current_dots = self.total_dots.get();
        let current_bus = self.open_bus.get();

        // Update the open bus value for the specified bits
        let new_bus = (current_bus & !mask) | (value & mask);
        self.open_bus.set(new_bus);

        // Refresh the timestamp for each bit that was set
        for bit in 0..8 {
            let bit_mask = 1u8 << bit;
            if (mask & bit_mask) != 0 {
                self.open_bus_refresh_dots[bit].set(current_dots);
            }
        }
    }

    /// Get the open bus value with decay applied
    fn get_decayed_open_bus(&self) -> u8 {
        let current_dots = self.total_dots.get();
        let current_bus = self.open_bus.get();
        let mut decayed_bus = 0u8;

        for bit in 0..8 {
            let bit_mask = 1u8 << bit;
            let refresh_time = self.open_bus_refresh_dots[bit].get();

            // Check if this bit has decayed
            let elapsed = current_dots.saturating_sub(refresh_time);
            if elapsed < OPEN_BUS_DECAY_DOTS {
                // Bit has not decayed, keep its value
                decayed_bus |= current_bus & bit_mask;
            }
            // If elapsed >= OPEN_BUS_DECAY_DOTS, bit has decayed to 0
        }

        decayed_bus
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
            // When reading from palette RAM ($3F00-$3FFF), the palette value is returned
            // directly (no buffering delay), BUT the read buffer is still updated with
            // the underlying nametable data at the mirrored address ($2F00-$2FFF).
            let nametable_addr = video_addr - 0x1000;
            self.register.borrow_mut().data = self.bus.read_byte(nametable_addr)?;

            // Palette RAM only stores 6-bit values (color indices 0-63).
            // The upper 2 bits of the read come from the PPU's decayed open bus.
            let palette_value = self.bus.read_byte(video_addr)? & 0x3F;
            let open_bus_upper = self.get_decayed_open_bus() & 0xC0;
            palette_value | open_bus_upper
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

    fn create_mirrored_name_tables_and_connect_to_bus(bus: &mut Box<dyn Bus>, mirroring: Rc<RefCell<PpuNameTableMirroring>>) -> Result<(), PpuError> {
        let ciram_memory = CiramMemory::new(mirroring);
        bus.add_device(Rc::new(RefCell::new(ciram_memory)))?;

        Ok(())
    }

    pub fn new(chr_rom: Rc<RefCell<dyn BusDevice>>, mirroring: Rc<RefCell<PpuNameTableMirroring>>, cpu: Rc<RefCell<dyn CPU>>, config: ConfigSpec) -> Result<Self, PpuError> {
        let mut bus: Box<dyn Bus> = Box::new(NESBus::new());

        let palette_table = Rc::new(RefCell::new(
            MemoryPalette::new(PALETTE_SIZE, PALETTE_ADDRESS_SPACE)));

        palette_table.borrow_mut().initialize()?;

        bus.add_device(palette_table)?;
        bus.add_device(chr_rom)?;

        Ppu2c02::create_mirrored_name_tables_and_connect_to_bus(&mut bus, mirroring)?;

        let pre_render = config.scanlines_per_frame - 1;
        let mut ppu = Ppu2c02 {
            register: RefCell::new(Register::new()),
            bus,
            v: RefCell::new(0),
            t: 0,
            x: 0,
            oam: OAM::default(),
            latch: RefCell::new(Latch::new()),
            renderer: RefCell::new(Renderer::new()),
            cpu,
            state: PpuState::VBlank(0),
            background_pixels_line: PixelLines::default(),
            sprites_pixels_line: PixelLines::default(),
            config,
            cycles_per_scanline_fp: 0,
            cycles_acc_fp: 0,
            nmi_suppressed: Cell::new(false),
            open_bus: Cell::new(0),
            total_dots: Cell::new(0),
            open_bus_refresh_dots: [
                Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0),
                Cell::new(0), Cell::new(0), Cell::new(0), Cell::new(0),
            ],
            // Initialize dot-level timing at start of pre-render scanline
            current_dot: 0,
            current_scanline: pre_render,
            scanline_rendered: false,
            // Sprite 0 hit tracking
            sprite0_hit_pending: false,
            sprite0_hit_x: 0,
            // Partial scanline rendering
            last_rendered_dot: 0,
            // Background shift registers (Phase 5)
            bg_shift_pattern_lo: 0,
            bg_shift_pattern_hi: 0,
            bg_shift_attrib_lo: 0,
            bg_shift_attrib_hi: 0,
            bg_next_tile_id: 0,
            bg_next_tile_attrib: 0,
            bg_next_tile_lo: 0,
            bg_next_tile_hi: 0,
            // Start with scanline rendering (can switch to dot-level later)
            dot_level_rendering: true,
        };

        ppu.recompute_timing();
        ppu.state = PpuState::VBlank(ppu.pre_render_scanline());

        //debug!("created PPU 2C02 with config: {:?}", ppu.config);
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

    /// Test helper: get current dot position (0-340)
    #[cfg(test)]
    pub fn get_current_dot(&self) -> u16 {
        self.current_dot
    }

    /// Test helper: get current scanline (0-261 for NTSC)
    #[cfg(test)]
    pub fn get_current_scanline(&self) -> u16 {
        self.current_scanline
    }

    /// Test helper: check if VBlank flag is set
    #[cfg(test)]
    pub fn is_vblank_set(&self) -> bool {
        self.get_flag(Status(VBlank))
    }

    /// Test helper: check if Sprite 0 Hit flag is set
    #[cfg(test)]
    pub fn is_sprite0_hit_set(&self) -> bool {
        self.get_flag(Status(Sprite0Hit))
    }

    // =========================================================================
    // Shift Register Test Helpers (Phase 5 - Dot-Level Rendering)
    // =========================================================================

    /// Test helper: get background pattern shift register (low plane)
    #[cfg(test)]
    pub fn get_bg_shift_pattern_lo(&self) -> u16 {
        self.bg_shift_pattern_lo
    }

    /// Test helper: get background pattern shift register (high plane)
    #[cfg(test)]
    pub fn get_bg_shift_pattern_hi(&self) -> u16 {
        self.bg_shift_pattern_hi
    }

    /// Test helper: get background attribute shift register (low plane)
    #[cfg(test)]
    pub fn get_bg_shift_attrib_lo(&self) -> u16 {
        self.bg_shift_attrib_lo
    }

    /// Test helper: get background attribute shift register (high plane)
    #[cfg(test)]
    pub fn get_bg_shift_attrib_hi(&self) -> u16 {
        self.bg_shift_attrib_hi
    }

    /// Test helper: set background pattern shift register (low plane)
    #[cfg(test)]
    pub fn set_bg_shift_pattern_lo(&mut self, value: u16) {
        self.bg_shift_pattern_lo = value;
    }

    /// Test helper: set background pattern shift register (high plane)
    #[cfg(test)]
    pub fn set_bg_shift_pattern_hi(&mut self, value: u16) {
        self.bg_shift_pattern_hi = value;
    }

    /// Test helper: set background attribute shift register (low plane)
    #[cfg(test)]
    pub fn set_bg_shift_attrib_lo(&mut self, value: u16) {
        self.bg_shift_attrib_lo = value;
    }

    /// Test helper: set background attribute shift register (high plane)
    #[cfg(test)]
    pub fn set_bg_shift_attrib_hi(&mut self, value: u16) {
        self.bg_shift_attrib_hi = value;
    }

    /// Test helper: set next tile pattern low byte
    #[cfg(test)]
    pub fn set_bg_next_tile_lo(&mut self, value: u8) {
        self.bg_next_tile_lo = value;
    }

    /// Test helper: set next tile pattern high byte
    #[cfg(test)]
    pub fn set_bg_next_tile_hi(&mut self, value: u8) {
        self.bg_next_tile_hi = value;
    }

    /// Test helper: set next tile attribute
    #[cfg(test)]
    pub fn set_bg_next_tile_attrib(&mut self, value: u8) {
        self.bg_next_tile_attrib = value;
    }

    /// Test helper: set fine X scroll
    #[cfg(test)]
    pub fn set_fine_x(&mut self, value: u8) {
        self.x = value & 0x07;
    }

    /// Test helper: get flag value for testing mid-scanline effects
    #[cfg(test)]
    pub fn get_flag_for_test(&self, flag_type: PpuFlagType) -> bool {
        match flag_type {
            PpuFlagType::ShowBackground => self.get_flag(Mask(ShowBackground)),
            PpuFlagType::ShowSprites => self.get_flag(Mask(ShowSprites)),
            PpuFlagType::VBlank => self.get_flag(Status(VBlank)),
            PpuFlagType::Sprite0Hit => self.get_flag(Status(Sprite0Hit)),
        }
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

    /// Fetch only a single line of pattern data - used for MMC2/MMC4 compatibility
    /// where reading all 8 lines at once would trigger latch incorrectly
    fn fetch_single_line_pattern_data_from_bus(&self, tile_index: u8, pattern_table_addr: u16, line: u8, flip_horizontal: bool) -> Result<Vec<u8>, PpuError> {
        let base_addr = pattern_table_addr + (tile_index as u16 * PATTERN_DATA_SIZE as u16);
        let mut pattern_data0 = self.bus.read_byte(base_addr + line as u16)?;
        let mut pattern_data1 = self.bus.read_byte(base_addr + line as u16 + (PATTERN_DATA_SIZE as u16 / 2))?;

        if flip_horizontal {
            self.flip_horizontal(&mut pattern_data0, &mut pattern_data1);
        }

        Ok(self.merge_bit_planes(&mut pattern_data0, &mut pattern_data1))
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

    /// Detect sprite 0 hit and store the position for dot-accurate triggering.
    /// The actual flag is set in advance_dots() when we reach the hit position.
    /// Sprite 0 hit can only occur on dots 2-254 (not at dots 0-1 or 255).
    fn detect_sprite_0_hit_and_store_position(&mut self, pixel_pos_x: u8) {
        // Sprite 0 hit can't occur at x=255
        if pixel_pos_x == PIXEL_X_MAX {
            return;
        }

        // Already have a pending hit, don't overwrite with later position
        if self.sprite0_hit_pending {
            return;
        }

        // Check left column masking
        if pixel_pos_x < 8 {
            let bg_left_enabled = self.get_flag(Mask(MaskFlag::ShowLeftmostBackground));
            let sprite_left_enabled = self.get_flag(Mask(MaskFlag::ShowLeftmostSprites));

            if !bg_left_enabled || !sprite_left_enabled {
                return;
            }
        }

        let background_transparency = self.background_pixels_line.is_transparent(pixel_pos_x);
        let sprite_transparency = self.sprites_pixels_line.is_transparent(pixel_pos_x);

        if !sprite_transparency && !background_transparency {
            // Store the hit position - add 2 because sprite 0 hit can't occur at dots 0-1
            // The hit occurs at dot = pixel_x + 2 (accounting for PPU render pipeline)
            self.sprite0_hit_pending = true;
            self.sprite0_hit_x = (pixel_pos_x as u16).saturating_add(2).min(255);
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
                        self.detect_sprite_0_hit_and_store_position(pixel_pos_x_plus_pixel);
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
        let tile = self.fetch_tile(coarse_x, coarse_y, name_table_addr, pattern_table_addr, attribute_table_addr)?;
        Ok(Rc::new(tile))
    }

    /***
     * coarse_x: ...ABCDE <- v: ........ ...ABCDE
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
        let mut v = *self.v.borrow();
        v = (v & !0x7BE0) | (self.t & 0x7BE0);
        *self.v.borrow_mut() = v;
    }

    // =========================================================================
    // Background shift register operations (Phase 5: Dot-level rendering)
    // =========================================================================

    /// Shift all background shift registers left by 1 bit.
    /// Called every dot during visible pixels (dots 1-256) and prefetch (dots 321-336).
    pub(crate) fn bg_shift_registers(&mut self) {
        self.bg_shift_pattern_lo <<= 1;
        self.bg_shift_pattern_hi <<= 1;
        self.bg_shift_attrib_lo <<= 1;
        self.bg_shift_attrib_hi <<= 1;
    }

    /// Load the next tile data into the lower 8 bits of the shift registers.
    /// Called every 8 dots when the shift registers need to be refilled.
    /// Note: Data is loaded into lower bits, then shifted left over the next 8 dots.
    pub(crate) fn bg_load_shift_registers(&mut self) {
        // Load pattern data into lower 8 bits
        self.bg_shift_pattern_lo = (self.bg_shift_pattern_lo & 0xFF00) | (self.bg_next_tile_lo as u16);
        self.bg_shift_pattern_hi = (self.bg_shift_pattern_hi & 0xFF00) | (self.bg_next_tile_hi as u16);

        // Attribute bits are the same for all 8 pixels of a tile
        // Bit 0 of attribute goes to all bits of attrib_lo lower byte
        // Bit 1 of attribute goes to all bits of attrib_hi lower byte
        let attrib_lo_fill: u16 = if self.bg_next_tile_attrib & 0x01 != 0 { 0x00FF } else { 0x0000 };
        let attrib_hi_fill: u16 = if self.bg_next_tile_attrib & 0x02 != 0 { 0x00FF } else { 0x0000 };
        self.bg_shift_attrib_lo = (self.bg_shift_attrib_lo & 0xFF00) | attrib_lo_fill;
        self.bg_shift_attrib_hi = (self.bg_shift_attrib_hi & 0xFF00) | attrib_hi_fill;
    }

    /// Get the background pixel color (0-3) at the current fine X position.
    /// The fine X scroll selects which bit of the shift registers to use.
    pub(crate) fn bg_get_pixel_color(&self) -> u8 {
        // Fine X scroll determines which bit to select (0-7)
        // We select from the high bits of the 16-bit registers
        // bit_select = 15 - fine_x (so fine_x=0 selects bit 15, fine_x=7 selects bit 8)
        let bit_select = 15 - self.x;
        let bit_mask = 1u16 << bit_select;

        let p0 = ((self.bg_shift_pattern_lo & bit_mask) >> bit_select) as u8;
        let p1 = ((self.bg_shift_pattern_hi & bit_mask) >> bit_select) as u8;
        let a0 = ((self.bg_shift_attrib_lo & bit_mask) >> bit_select) as u8;
        let a1 = ((self.bg_shift_attrib_hi & bit_mask) >> bit_select) as u8;

        // Combine: palette_index = (attribute << 2) | pattern
        (a1 << 3) | (a0 << 2) | (p1 << 1) | p0
    }

    /// Fetch the next tile ID from the nametable.
    /// Address = 0x2000 | (v & 0x0FFF)
    fn bg_fetch_tile_id(&mut self) -> Result<(), PpuError> {
        let v = *self.v.borrow();
        let addr = 0x2000 | (v & 0x0FFF);
        self.bg_next_tile_id = self.bus.read_byte(addr)?;
        Ok(())
    }

    /// Fetch the attribute byte for the current tile.
    /// Address = 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07)
    fn bg_fetch_attribute(&mut self) -> Result<(), PpuError> {
        let v = *self.v.borrow();
        let addr = 0x23C0 | (v & 0x0C00) | ((v >> 4) & 0x38) | ((v >> 2) & 0x07);
        let attrib_byte = self.bus.read_byte(addr)?;

        // The attribute byte contains 4 2-bit palette selections for a 4x4 tile area
        // We need to extract the correct 2 bits based on coarse X and coarse Y
        let coarse_x = v & 0x001F;
        let coarse_y = (v >> 5) & 0x001F;

        // Determine which quadrant (2x2 tiles) we're in
        // bit 1 of coarse_x and coarse_y determine the shift
        let shift = ((coarse_y & 0x02) << 1) | (coarse_x & 0x02);
        self.bg_next_tile_attrib = (attrib_byte >> shift) & 0x03;
        Ok(())
    }

    /// Fetch the low byte of the pattern table for the current tile.
    /// Address = (control.background_pattern << 12) | (tile_id << 4) | fine_y
    fn bg_fetch_pattern_lo(&mut self) -> Result<(), PpuError> {
        let v = *self.v.borrow();
        let fine_y = (v >> 12) & 0x07;
        let pattern_table = if self.get_flag(Control(BackgroundPatternTableAddr)) { 0x1000u16 } else { 0x0000u16 };
        let addr = pattern_table | ((self.bg_next_tile_id as u16) << 4) | fine_y;
        self.bg_next_tile_lo = self.bus.read_byte(addr)?;
        Ok(())
    }

    /// Fetch the high byte of the pattern table for the current tile.
    /// Address = (control.background_pattern << 12) | (tile_id << 4) | fine_y + 8
    fn bg_fetch_pattern_hi(&mut self) -> Result<(), PpuError> {
        let v = *self.v.borrow();
        let fine_y = (v >> 12) & 0x07;
        let pattern_table = if self.get_flag(Control(BackgroundPatternTableAddr)) { 0x1000u16 } else { 0x0000u16 };
        let addr = pattern_table | ((self.bg_next_tile_id as u16) << 4) | fine_y | 0x08;
        self.bg_next_tile_hi = self.bus.read_byte(addr)?;
        Ok(())
    }

    /// Increment the coarse X position in the v register.
    /// Called at the end of each 8-dot tile fetch cycle.
    fn bg_increment_coarse_x(&mut self) {
        let mut v = *self.v.borrow();
        if (v & 0x001F) == 31 {
            // Wrap around and switch horizontal nametable
            v = (v & !0x001F) ^ 0x0400;
        } else {
            v += 1;
        }
        *self.v.borrow_mut() = v;
    }

    /// Increment the fine Y and coarse Y position in the v register.
    /// Called at the end of each scanline (dot 256).
    fn bg_increment_y(&mut self) {
        let mut v = *self.v.borrow();
        if (v & 0x7000) != 0x7000 {
            // Fine Y < 7, just increment
            v += 0x1000;
        } else {
            // Fine Y = 7, reset to 0 and increment coarse Y
            v &= !0x7000;
            let mut coarse_y = (v & 0x03E0) >> 5;
            if coarse_y == 29 {
                // Row 29 is the last row of tiles, wrap to 0 and switch vertical nametable
                coarse_y = 0;
                v ^= 0x0800;
            } else if coarse_y == 31 {
                // Coarse Y = 31 wraps to 0 but doesn't switch nametable
                coarse_y = 0;
            } else {
                coarse_y += 1;
            }
            v = (v & !0x03E0) | (coarse_y << 5);
        }
        *self.v.borrow_mut() = v;
    }

    /// Render a single dot for the current scanline (Phase 5: Dot-level rendering).
    ///
    /// This method implements per-dot pixel output and tile fetching.
    /// It is called for each dot during visible scanlines (0-239) when dot_level_rendering is enabled.
    ///
    /// Timing overview for dots 1-256:
    /// - Each dot: Shift registers shift, output one pixel
    /// - Every 8 dots (at dots 9, 17, 25, ...): Load new tile into shift registers
    /// - Tile fetch cycle (dots 1-8 of each tile):
    ///   - Dots 1-2: Fetch nametable byte
    ///   - Dots 3-4: Fetch attribute byte
    ///   - Dots 5-6: Fetch pattern low byte
    ///   - Dots 7-8: Fetch pattern high byte, then load shift registers
    ///
    /// Mid-scanline effects (Phase 5.4):
    /// - PPUMASK flags are read fresh at each dot for pixel output
    /// - Scroll register changes affect tile fetches immediately
    /// - Sprite 0 hit is evaluated with current flag state
    fn render_dot(&mut self, scanline: u16, dot: u16, _show_background: bool, _show_sprites: bool) -> Result<(), PpuError> {
        // Read PPUMASK flags fresh at each dot for mid-scanline effects
        let show_background = self.get_flag(Mask(ShowBackground));
        let show_sprites = self.get_flag(Mask(ShowSprites));
        // Dots 1-256: Visible pixel output
        if dot <= 256 {
            let pixel_x = (dot - 1) as u8;  // dot 1 = pixel 0, dot 256 = pixel 255

            // First dot of scanline: sprite evaluation (tiles already prefetched from dots 321-336)
            if dot == 1 {
                // NOTE: Shift registers already contain tile data from prefetch cycles (dots 321-336)
                // of the previous scanline. Do NOT reset them here.

                // Start fetching tile 3 (tiles 1-2 are already in shift registers from prefetch)
                // Fetch all data now (simplified), will be loaded at dot 8
                self.bg_fetch_tile_id()?;
                self.bg_fetch_attribute()?;
                self.bg_fetch_pattern_lo()?;
                self.bg_fetch_pattern_hi()?;

                // Do sprite evaluation for this scanline
                self.do_sprite_evaluation(scanline)?;

                // Render sprites for comparison (still using scanline-based for now)
                if show_sprites {
                    self.render_sprites(scanline)?;
                }
            }

            // Output pixel from shift registers
            let bg_pixel = if show_background {
                self.bg_get_pixel_color()
            } else {
                0
            };

            // Get background color from palette
            let bg_color = self.get_palette_color(bg_pixel, false)?;

            // Check sprite pixel at this position
            let sprite_pixel = &self.sprites_pixels_line.rgba_pixels[pixel_x as usize];
            let sprite_color_index = self.get_sprite_pixel_color_index(pixel_x);

            // Pixel priority logic
            let (final_r, final_g, final_b) = if sprite_color_index != 0 && show_sprites {
                // Sprite is non-transparent
                if bg_pixel == 0 {
                    // Background is transparent, show sprite
                    (sprite_pixel.r, sprite_pixel.g, sprite_pixel.b)
                } else if sprite_pixel.priority == SpritePriority::Front {
                    // Sprite has front priority, show sprite
                    (sprite_pixel.r, sprite_pixel.g, sprite_pixel.b)
                } else {
                    // Sprite has back priority and BG is non-transparent, show BG
                    bg_color
                }
            } else {
                // Sprite is transparent or sprites disabled, show background
                bg_color
            };

            // Check for sprite 0 hit
            if !self.get_flag(Status(Sprite0Hit))
                && show_background
                && show_sprites
                && bg_pixel != 0
                && sprite_color_index != 0
            {
                // Check if this is sprite 0
                if self.is_sprite_0_at_x(pixel_x) {
                    // Sprite 0 hit! But don't set on leftmost 8 pixels if clipping
                    let clip_left = !self.get_flag(Mask(MaskFlag::ShowLeftmostBackground))
                        || !self.get_flag(Mask(MaskFlag::ShowLeftmostSprites));
                    if !(clip_left && pixel_x < 8) && pixel_x != 255 {
                        self.set_flag(Status(Sprite0Hit), true);
                    }
                }
            }

            // Write pixel to frame
            self.renderer.borrow_mut().frame_as_mut().set_pixel(
                pixel_x,
                scanline as u8,
                (final_r, final_g, final_b),
            );

            // Shift registers shift every dot
            self.bg_shift_registers();

            // Every 8 dots: Load new tile data and increment coarse X
            if dot % 8 == 0 && dot < 256 {
                self.bg_load_shift_registers();
                self.bg_increment_coarse_x();

                // Fetch next tile into latches
                self.bg_fetch_tile_id()?;
                self.bg_fetch_attribute()?;
                self.bg_fetch_pattern_lo()?;
                self.bg_fetch_pattern_hi()?;
            }

            // Dot 256: Increment Y
            if dot == 256 {
                self.bg_increment_y();
            }
        }

        // Dot 257: Copy horizontal bits from t to v, reset OAMADDR
        if dot == 257 {
            self.put_horizontal_t_into_v();
            self.register.borrow_mut().oam_addr = 0;
        }

        // Dots 257-320: Sprite tile fetches
        // 8 sprites × 8 cycles each = 64 cycles
        // Each sprite: garbage NT (2), garbage AT (2), pattern lo (2), pattern hi (2)
        // These fetches are important for MMC2/MMC4 mappers
        if dot >= 257 && dot <= 320 {
            // Note: Actual sprite pattern fetches happen here on real hardware
            // For now, we do sprite rendering at dot 1 (atomic), but we still
            // perform the fetches for mapper compatibility if needed
            // The sprite_cycle within the 64-dot window determines which sprite
            let _sprite_cycle = dot - 257;
            // Future: implement per-sprite pattern fetches here
        }

        // Dots 321-336: Prefetch first two tiles of next scanline
        // Same fetch pattern as dots 1-16
        if dot >= 321 && dot <= 336 {
            let prefetch_dot = dot - 320;  // Maps 321-336 to 1-16

            // Shift registers shift every dot during prefetch
            self.bg_shift_registers();

            // Every 8 dots: complete a tile fetch cycle
            if prefetch_dot == 8 {
                // First tile complete - load into shift registers
                self.bg_load_shift_registers();
                self.bg_increment_coarse_x();

                // Start fetching second tile
                self.bg_fetch_tile_id()?;
                self.bg_fetch_attribute()?;
                self.bg_fetch_pattern_lo()?;
                self.bg_fetch_pattern_hi()?;
            } else if prefetch_dot == 16 {
                // Second tile complete - load into shift registers
                self.bg_load_shift_registers();
                self.bg_increment_coarse_x();
            } else if prefetch_dot == 1 {
                // Start first tile fetch
                self.bg_fetch_tile_id()?;
                self.bg_fetch_attribute()?;
                self.bg_fetch_pattern_lo()?;
                self.bg_fetch_pattern_hi()?;
            }
        }

        // Dots 337-340: Dummy nametable fetches (2 fetches × 2 cycles each)
        // These are unused fetches that just read the nametable
        if dot == 337 || dot == 339 {
            let _ = self.bg_fetch_tile_id();  // Dummy fetch, result unused
        }

        // Mark scanline as rendered after dot 256
        if dot == 256 && !self.scanline_rendered {
            self.scanline_rendered = true;
            self.last_rendered_dot = VISIBLE_DOTS;
            self.state = PpuState::Rendering(scanline + 1);
        }

        Ok(())
    }

    /// Get the color index for a sprite pixel at the given X position.
    /// Returns 0 if transparent, or the palette index (1-3) if opaque.
    fn get_sprite_pixel_color_index(&self, x: u8) -> u8 {
        let pixel = &self.sprites_pixels_line.rgba_pixels[x as usize];
        // Check if sprite pixel is transparent (priority == None means no sprite)
        if pixel.priority == SpritePriority::None {
            0
        } else {
            // Return non-zero to indicate opaque sprite
            // The actual color is already stored in the pixel
            1
        }
    }

    /// Check if sprite 0 is at the given X position.
    fn is_sprite_0_at_x(&self, x: u8) -> bool {
        // Check secondary OAM for sprite 0
        for i in 0..self.oam.sprite_count {
            let sprite = &self.oam.secondary[i];
            if sprite.sprite0 && x >= sprite.x && x < sprite.x.saturating_add(SPRITE_WIDTH) {
                return true;
            }
        }
        false
    }

    /// Get a color from the palette for dot-level rendering.
    /// Returns (r, g, b) tuple.
    ///
    /// The color_index is a 4-bit value:
    /// - bits 0-1: pattern table color (0-3)
    /// - bits 2-3: palette selection (0-3)
    ///
    /// For background: palette address = 0x3F00 + (palette << 2) + pattern_color
    /// Color 0 always uses the universal background color at 0x3F00.
    fn get_palette_color(&self, color_index: u8, _is_sprite: bool) -> Result<(u8, u8, u8), PpuError> {
        let pattern_color = color_index & 0x03;
        let palette = (color_index >> 2) & 0x03;

        let palette_addr = if pattern_color == 0 {
            // Color 0 is always the universal background color
            0x3F00u16
        } else {
            // Background palette: 0x3F00 + (palette * 4) + pattern_color
            0x3F00 + ((palette as u16) << 2) + (pattern_color as u16)
        };

        let nes_color_index = self.bus.read_byte(palette_addr)? & 0x3F;
        let (r, g, b, _a) = Palette2C02::rgba_opaque(nes_color_index);
        Ok((r, g, b))
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

        //let pattern_table_addr = self.get_background_pattern_table_addr();
        let mut attribute_table_addr = self.get_attribute_table_addr(name_table_addr);

        let mut fine_y = self.get_fine_y();
        let mut coarse_y = self.get_coarse_y();

        let mut coarse_x = self.get_coarse_x();
        let pixel_pos_y = scanline;

        let mut fine_x = self.get_fine_x();

        self.background_pixels_line.clear();

        let mut pixel_pos_x= 0u8;

        loop {
            let pattern_table_addr = self.get_background_pattern_table_addr();
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

        // simulate prefetch two more tiles (off-screen), to trigger mapper CHR reads
        let prefetch_tiles = 2;
        for _ in 0..prefetch_tiles {
            let pattern_table_addr = self.get_background_pattern_table_addr();
            let tile = self.get_tile(coarse_x, coarse_y, name_table_addr, pattern_table_addr, attribute_table_addr)?;

            let _ = self.fetch_line_pattern_data(tile.as_ref(), fine_y, 0, 8);

            (name_table_addr, coarse_x) = self.coarse_x_increment(name_table_addr, coarse_x);
            attribute_table_addr = self.get_attribute_table_addr(name_table_addr);
        }
        // pre fetch

        name_table_addr = self.get_name_table_addr_from_v();
        (name_table_addr, fine_y, coarse_y) = self.fine_and_coarse_y_increment(name_table_addr, fine_y, coarse_y);

        self.update_v_coarse_x(name_table_addr, coarse_x);
        self.update_v_fine_and_coarse_y(name_table_addr, fine_y, coarse_y);

        //trace!("PPU: rendered background for scanline: {}", scanline);
        Ok(())
    }

    fn is_scanline_in_sprite_range(&self, scanline: u16, sprite: &Sprite, size: u8) -> bool {
        let top = sprite.y as u16 + 1;
        let bottom = top + size as u16;

        scanline >= top && scanline < bottom
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

        // Fetch only the single line needed - critical for MMC2/MMC4 latch accuracy
        // Reading all 8 lines would trigger the latch on line 0's high byte ($xFD8/$xFE8)
        // even when rendering a different line
        let line_data = self.fetch_single_line_pattern_data_from_bus(tile_index, fixed_pattern_table_addr, tile_offset, flip_horizontal)?;

        // Create tile with only the needed line at position 0
        let mut pattern_data = [0u8; MERGED_PATTERN_DATA_SIZE];
        pattern_data[0..8].copy_from_slice(&line_data);
        let tile = Tile::new(tile_index, colors, pattern_data);

        // Return tile_offset as 0 since the line data is at the beginning of pattern_data
        Ok((tile, 0))
    }

    fn do_sprite_evaluation(&mut self, scanline: u16) -> Result<(), PpuError> {
        self.oam.clear_secondary();
        let sprite_size = if self.get_flag(Control(SpriteSize)) { 16u8 } else { 8u8 };

        let mut count = 0usize;
        for i in 0..self.oam.primary.len() {
            let sprite = &self.oam.primary[i];

            if self.is_scanline_in_sprite_range(scanline, sprite, sprite_size) {
                if count < self.oam.secondary.len() {
                    self.oam.secondary[count] = *sprite;

                    if i == 0 {
                        self.oam.secondary[count].sprite0 = true;
                    }

                    count += 1;
                } else {
                    self.set_flag(Status(SpriteOverflow), true);
                    break;
                }
            }
        }

        self.oam.sprite_count = count;
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
        // Check if this is sprite 0 and if hit hasn't already occurred.
        // The actual mask flag check (ShowBackground && ShowSprites) is done
        // when setting the flag at the hit dot, allowing mid-scanline mask changes.
        if self.get_flag(Status(Sprite0Hit)) == false && is_sprite_0 {
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
        // Pre-fetched sprite data for MMC2/MMC4 latch compatibility
        // Pattern data must be fetched in ascending OAM order (0 to N-1) to trigger
        // latches in the correct order, matching real NES hardware behavior
        struct SpriteFetchData {
            tile: Tile,
            tile_offset: u8,
            sprite_x: u8,
            width: usize,
            sprite0: bool,
            priority: SpritePriority,
        }

        let is_sprite_8x16  = self.get_flag(Control(SpriteSize));
        let sprite_pattern_table_addr = self.get_sprites_pattern_table_addr();

        //trace!("rendering {} sprites for scanline: {}", self.oam.sprite_count, scanline);

        self.sprites_pixels_line.clear();

        // Pass 1: Fetch pattern data in ascending order (0 to N-1)
        // This ensures MMC2/MMC4 latch triggers happen in the correct order
        let mut fetched_sprites: Vec<SpriteFetchData> = Vec::with_capacity(self.oam.sprite_count);

        for i in 0..self.oam.sprite_count {
            let sprite = &self.oam.secondary[i];

            let pixel_pos_y = (scanline).wrapping_sub(sprite.y as u16 + 1) as u8;
            let width = (SPRITE_WIDTH as u16).min((PIXEL_X_MAX as u16).saturating_sub(sprite.x as u16) + 1) as usize;

            let (tile, tile_offset) = self.get_tile_by_sprite_definition(sprite, is_sprite_8x16, pixel_pos_y, sprite_pattern_table_addr)?;

            fetched_sprites.push(SpriteFetchData {
                tile,
                tile_offset,
                sprite_x: sprite.x,
                width,
                sprite0: sprite.sprite0,
                priority: self.get_sprite_priority(sprite),
            });
        }

        // Pass 2: Render pixels in descending order (N-1 to 0) for correct priority
        // Lower OAM index = higher priority, so render low priority sprites first
        for fetch_data in fetched_sprites.iter().rev() {
            let sprite0_hit_detect = self.detect_sprite_0_hit(fetch_data.sprite0);

            let line_pattern_data = self.fetch_line_pattern_data(&fetch_data.tile, fetch_data.tile_offset, 0, fetch_data.width);
            let palette = fetch_data.tile.colors;

            self.set_pixel(fetch_data.sprite_x, scanline as u8, &line_pattern_data, palette, PixelMode::Sprite, fetch_data.priority, sprite0_hit_detect);
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

    fn recompute_timing(&mut self) {
        let cycles_per_scanline = (PPU_DOTS_PER_SCANLINE as f64) * (self.config.cpu_clock_hz / self.config.ppu_clock_hz);
        self.cycles_per_scanline_fp = (cycles_per_scanline * FIXED_POINT_ONE as f64).round() as u64;
        self.cycles_acc_fp = 0;
    }

    /// Deprecated: Used by old scanline-based rendering.
    /// Kept for reference and potential debugging.
    #[allow(dead_code)]
    fn grant_cpu_cycles_for_scanline(&mut self) -> u16 {
        self.cycles_acc_fp = self.cycles_acc_fp.wrapping_add(self.cycles_per_scanline_fp);
        let whole = (self.cycles_acc_fp >> FIXED_POINT_SHIFT) as u16;
        self.cycles_acc_fp &= FIXED_POINT_ONE - 1;

        whole
    }

    #[inline]
    fn pre_render_scanline(&self) -> u16 {
        self.config.scanlines_per_frame - 1
    }

    #[inline]
    fn last_visible_scanline(&self) -> u16 {
        self.config.visible_scanlines - 1
    }

    /// Advance PPU by the specified number of dots (internal implementation).
    /// This method handles scanline transitions, VBlank/NMI timing,
    /// and triggers rendering at the appropriate times.
    /// Returns an optional frame when a frame is completed.
    fn advance_dots_internal(&mut self, dots: u32) -> Result<Option<NesFrame>, PpuError> {
        let mut frame_ready = false;

        for _ in 0..dots {
            // Advance to next dot and track total for open bus decay
            self.current_dot += 1;
            self.total_dots.set(self.total_dots.get().wrapping_add(1));

            // Handle end of scanline
            if self.current_dot >= DOTS_PER_SCANLINE {
                self.current_dot = 0;
                self.scanline_rendered = false;
                // Clear sprite 0 hit pending for new scanline
                self.sprite0_hit_pending = false;
                // Reset partial rendering tracker for new scanline
                self.last_rendered_dot = 0;

                // Move to next scanline
                self.current_scanline += 1;
                if self.current_scanline >= self.config.scanlines_per_frame {
                    self.current_scanline = 0;
                }
            }

            // Check for sprite 0 hit at the correct dot (only on visible scanlines)
            // Sprite 0 hit requires BOTH ShowBackground AND ShowSprites to be enabled
            // at the moment the hit occurs (allows for mid-scanline mask changes)
            if self.sprite0_hit_pending
                && self.current_scanline <= self.last_visible_scanline()
                && self.current_dot == self.sprite0_hit_x
                && !self.get_flag(Status(Sprite0Hit))
                && self.get_flag(Mask(ShowBackground))
                && self.get_flag(Mask(ShowSprites))
            {
                self.set_flag(Status(Sprite0Hit), true);
            }

            // Process events at specific dots
            match (self.current_scanline, self.current_dot) {
                // Pre-render scanline, dot 1: Clear flags
                (scanline, VBLANK_SET_DOT) if scanline == self.pre_render_scanline() => {
                    let show_bg = self.get_flag(Mask(ShowBackground));
                    let show_spr = self.get_flag(Mask(ShowSprites));
                    self.set_flag(Status(VBlank), false);
                    self.set_flag(Status(Sprite0Hit), false);
                    self.set_flag(Status(SpriteOverflow), false);
                    self.sprite0_hit_pending = false;
                    let _ = self.cpu.borrow_mut().clear_nmi();
                    self.nmi_suppressed.set(false);
                    self.register.borrow_mut().oam_addr = 0;

                    // Do sprite evaluation for scanline 0 if rendering enabled
                    if show_bg || show_spr {
                        self.put_horizontal_t_into_v();
                        self.put_vertical_t_into_v();
                        self.do_sprite_evaluation(0)?;
                    }

                    self.state = PpuState::Rendering(0);
                }

                // Pre-render scanline, dots 321-336: Prefetch tiles for scanline 0
                (scanline, dot) if scanline == self.pre_render_scanline() && dot >= 321 && dot <= 336 => {
                    let show_bg = self.get_flag(Mask(ShowBackground));
                    let show_spr = self.get_flag(Mask(ShowSprites));
                    let rendering_enabled = show_bg || show_spr;

                    if rendering_enabled && self.dot_level_rendering {
                        let prefetch_dot = dot - 320;  // Maps 321-336 to 1-16

                        // Shift registers shift every dot during prefetch
                        self.bg_shift_registers();

                        // Every 8 dots: complete a tile fetch cycle
                        if prefetch_dot == 8 {
                            // First tile complete - load into shift registers
                            self.bg_load_shift_registers();
                            self.bg_increment_coarse_x();

                            // Start fetching second tile
                            self.bg_fetch_tile_id()?;
                            self.bg_fetch_attribute()?;
                            self.bg_fetch_pattern_lo()?;
                            self.bg_fetch_pattern_hi()?;
                        } else if prefetch_dot == 16 {
                            // Second tile complete - load into shift registers
                            self.bg_load_shift_registers();
                            self.bg_increment_coarse_x();
                        } else if prefetch_dot == 1 {
                            // Start first tile fetch
                            self.bg_fetch_tile_id()?;
                            self.bg_fetch_attribute()?;
                            self.bg_fetch_pattern_lo()?;
                            self.bg_fetch_pattern_hi()?;
                        }
                    }
                }

                // NMI scanline (241 for NTSC), dot 1: Set VBlank, trigger NMI
                (scanline, VBLANK_SET_DOT) if scanline == self.config.nmi_scanline => {
                    self.renderer.borrow_mut().update();
                    self.renderer.borrow_mut().reset();
                    self.set_flag(Status(VBlank), true);
                    frame_ready = true;

                    if self.get_flag(Control(GenerateNmi)) && !self.nmi_suppressed.get() {
                        self.cpu.borrow_mut().signal_nmi()?;
                    }
                    self.nmi_suppressed.set(false);

                    self.state = PpuState::VBlank(scanline + 1);
                }

                // Visible scanlines (0-239): Handle per-dot or scanline-based rendering
                (scanline, dot) if scanline <= self.last_visible_scanline() && dot >= 1 => {
                    let show_background = self.get_flag(Mask(ShowBackground));
                    let show_sprites = self.get_flag(Mask(ShowSprites));
                    let rendering_enabled = show_background || show_sprites;

                    if self.dot_level_rendering && rendering_enabled {
                        // Per-dot rendering mode (Phase 5)
                        self.render_dot(scanline, dot, show_background, show_sprites)?;
                    } else if !self.scanline_rendered && dot >= 1 {
                        // Fallback: Scanline-based rendering (original behavior)
                        if rendering_enabled {
                            self.render_background(scanline)?;
                        }

                        if show_sprites {
                            self.render_sprites(scanline)?;
                        }

                        if rendering_enabled {
                            self.do_sprite_evaluation(scanline + 1)?;
                            self.put_horizontal_t_into_v();
                        } else {
                            self.oam.clear_secondary();
                        }

                        self.write_pixels_lines_to_frame(scanline, show_background, show_sprites);
                        self.register.borrow_mut().oam_addr = 0;
                        self.scanline_rendered = true;
                        self.last_rendered_dot = VISIBLE_DOTS;
                        self.state = PpuState::Rendering(scanline + 1);
                    }
                }

                // Post-render scanline (240): Just update state
                (scanline, 1) if scanline == self.config.visible_scanlines && scanline < self.config.nmi_scanline => {
                    self.state = PpuState::Rendering(self.config.nmi_scanline);
                }

                // VBlank scanlines: Just advance state
                (scanline, 1) if scanline > self.config.nmi_scanline && scanline < self.pre_render_scanline() => {
                    self.state = PpuState::VBlank(scanline + 1);
                }

                _ => {}
            }
        }

        if frame_ready {
            Ok(Some(self.frame()))
        } else {
            Ok(None)
        }
    }

    /// Deprecated: Used by old scanline-based rendering.
    /// Kept for reference and potential debugging.
    #[allow(dead_code)]
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
            // pre render line: last line of the frame and go to Rendering(0)
            PpuState::VBlank(scanline) if scanline == self.pre_render_scanline() => {
                self.set_flag(Status(VBlank), false);
                self.set_flag(Status(Sprite0Hit), false);
                self.set_flag(Status(SpriteOverflow), false);

                let _ = self.cpu.borrow_mut().clear_nmi();
                self.nmi_suppressed.set(false);

                self.register.borrow_mut().oam_addr = 0;
                self.state = PpuState::Rendering(0);

                if self.get_flag(Mask(ShowBackground)) || self.get_flag(Mask(ShowSprites)) {
                    self.put_horizontal_t_into_v();
                    self.put_vertical_t_into_v();
                    self.do_sprite_evaluation(0)?;
                }
            },

            // rendering phase (scanlines 0 ... visible-1)
            PpuState::Rendering(scanline) if scanline <= self.last_visible_scanline() => {
                let show_background = self.get_flag(Mask(ShowBackground));
                let show_sprites = self.get_flag(Mask(ShowSprites));

                // Background tile fetches occur when either background OR sprites are enabled
                if show_background || show_sprites {
                    self.render_background(scanline)?;
                }

                if show_sprites {
                    self.render_sprites(scanline)?;
                }

                if show_background || show_sprites {
                    self.do_sprite_evaluation(scanline + 1)?;
                    self.put_horizontal_t_into_v();
                } else {
                    self.oam.clear_secondary();
                }

                self.write_pixels_lines_to_frame(scanline, show_background, show_sprites);

                self.register.borrow_mut().oam_addr = 0;
                self.state = PpuState::Rendering(scanline + 1);
            },

            // post render
            PpuState::Rendering(scanline) if scanline < self.config.nmi_scanline => {
                self.renderer.borrow_mut().update();
                self.state = PpuState::Rendering(self.config.nmi_scanline);
            },

            // post render: NMI
            PpuState::Rendering(scanline) if scanline == self.config.nmi_scanline => {
                self.renderer.borrow_mut().reset();
                self.set_flag(Status(VBlank), true);
                self.state = PpuState::VBlank(scanline + 1);

                if self.get_flag(Control(GenerateNmi)) && !self.nmi_suppressed.get() {
                    self.cpu.borrow_mut().signal_nmi()?;
                }

                self.nmi_suppressed.set(false);
            },

            // vblank
            PpuState::VBlank(scanline) if scanline < self.pre_render_scanline() => {
                self.state = PpuState::VBlank(scanline + 1);
            },

            _ => unreachable!("render_scanline()")
        }

        //trace!("PPU: scanline ending: {}", self.state);
        Ok(())
    }

    /// Deprecated: Used by old scanline-based rendering.
    /// Kept for reference and potential debugging.
    #[allow(dead_code)]
    fn render(&mut self) -> Result<u16, PpuError> {
        self.render_scanline()?;
        Ok(self.grant_cpu_cycles_for_scanline())
    }
}