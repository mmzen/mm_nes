use crate::ines_loader::Region;

/***
 * This struct represents the configuration settings for the NES console, mainly defined by the region.
 * The main differences between the NTSC / PAL / Dendy regions are depicted in the following table:
 *
 * https://www.nesdev.org/wiki/Cycle_reference_chart
 *
 ***/

pub trait Configurable {
    fn set_config(&mut self, config: ConfigSpec);
}

const NOISE_PERIOD_TABLE_NTSC: [u16; 16] = [
    4, 8, 16, 32, 64, 96, 128, 160, 202, 254, 380, 508, 762, 1016, 2034, 4068
];

const DMC_PERIOD_TABLE_NTSC: [u16; 16] = [
    428, 380, 340, 320, 286, 254, 226, 214, 190, 160, 142, 128, 106, 84, 72, 54
];

const NOISE_PERIOD_TABLE_PAL: [u16; 16] = [
    4, 8, 14, 16, 32, 64, 94, 128, 160, 202, 254, 380, 508, 762, 1016, 2034
];

const DMC_PERIOD_TABLE_PAL: [u16; 16] = [
    398, 354, 316, 298, 276, 236, 210, 198, 176, 148, 132, 118, 98, 78, 66, 50
];

#[derive(Clone, Debug)]
pub struct ConfigSpec {
    pub region: Region,

    pub master_clock_hz: f64,           // 21.477272 MHz (NTSC), 26.601712 MHz (PAL)
    pub cpu_clock_hz: f64,              // derived from master / divider
    pub ppu_clock_hz: f64,              // derived from master / divider
    pub apu_clock_hz: f64,              // derived from master / divider

    pub scanlines_per_frame: u16,       // 262 (NTSC), 312 (PAL/Dendy)
    pub visible_scanlines: u16,         // 240 (NTSC), 239 (PAL/Dendy)
    pub post_render_lines: u16,         // 1 (NTSC/PAL), 51 (Dendy)
    pub vblank_lines_after_nmi: u16,    // 20 (NTSC/Dendy), 70 (PAL)
    pub nmi_scanline: u16,              // 241 (NTSC), 240 (PAL), 291 (Dendy)

    pub apu_frame_counter_rate_hz: f64, // ~60 (NTSC), ~50 (PAL), ~59 (Dendy)
    pub cycles_per_sample: f64,
    pub dmc_period_table: &'static [u16; 16],   // region-specific tables
    pub noise_period_table: &'static [u16; 16], // region-specific tables

    pub pal_emphasis_rg_swapped: bool,  // true for PAL PPUs

    // Default palette for region
    // pub palette_64: &'static [u32; 64],
}

impl Default for ConfigSpec {
    fn default() -> ConfigSpec {
        ConfigSpec::ntsc()
    }
}

impl ConfigSpec {

    fn ntsc() -> ConfigSpec {
        ConfigSpec {
            region: Region::NTSC,
            master_clock_hz: 21_477_272.0,
            cpu_clock_hz: 21_477_272.0 / 12.0,
            ppu_clock_hz: 21_477_272.0 / 12.0 * 3.0,
            apu_clock_hz: 21_477_272.0 / 12.0 / 2.0,
            scanlines_per_frame: 262,
            visible_scanlines: 240,
            post_render_lines: 1,
            vblank_lines_after_nmi: 20,
            nmi_scanline: 241,
            apu_frame_counter_rate_hz: 60.0,
            cycles_per_sample: 21_477_272.0 / 12.0 / 2.0 / 44_100.0,
            dmc_period_table: &DMC_PERIOD_TABLE_NTSC,
            noise_period_table: &NOISE_PERIOD_TABLE_NTSC,
            pal_emphasis_rg_swapped: false,
        }
    }

    fn pal() -> ConfigSpec {
        ConfigSpec {
            region: Region::PAL,
            master_clock_hz: 26_601_712.0,
            cpu_clock_hz:    26_601_712.0 / 16.0,
            ppu_clock_hz:    26_601_712.0 / 5.0,
            apu_clock_hz:    26_601_712.0 / 16.0 / 2.0,
            scanlines_per_frame: 312,
            visible_scanlines:   239,
            post_render_lines:   1,
            vblank_lines_after_nmi: 70,
            nmi_scanline:        240,
            apu_frame_counter_rate_hz: 50.0,
            cycles_per_sample: 26_601_712.0 / 15.0 / 2.0 / 44_100.0,
            dmc_period_table:    &DMC_PERIOD_TABLE_PAL,
            noise_period_table:  &NOISE_PERIOD_TABLE_PAL,
            pal_emphasis_rg_swapped: true,
        }
    }

    fn dendy() -> ConfigSpec {
        ConfigSpec {
            region: Region::Dendy,
            master_clock_hz: 26_601_712.0,
            cpu_clock_hz:    26_601_712.0 / 15.0,
            ppu_clock_hz:    26_601_712.0 / 5.0,
            apu_clock_hz:    26_601_712.0 / 15.0 / 2.0,
            scanlines_per_frame: 312,
            visible_scanlines:   239,
            post_render_lines:   51,
            vblank_lines_after_nmi: 20,
            nmi_scanline:        291,
            apu_frame_counter_rate_hz: 59.0,
            cycles_per_sample: 26_601_712.0 / 16.0 / 2.0 / 44_100.0,
            dmc_period_table:    &DMC_PERIOD_TABLE_NTSC,
            noise_period_table:  &NOISE_PERIOD_TABLE_NTSC,
            pal_emphasis_rg_swapped: true,
        }
    }

    pub fn from_region(region: Region) -> ConfigSpec {
        match region {
            Region::NTSC => ConfigSpec::ntsc(),
            Region::PAL => ConfigSpec::pal(),
            Region::Dendy => ConfigSpec::dendy(),
            _ => { panic!("unsupported region: {:?}", region); }
        }
    }
}