// Authorship: Human 0% | Claude 100%
//! Tests for PPU VBlank timing (CPU-visible).
//!
//! Reproduces the ROM failure: "VBlank Beginning - The PPU Register $2002 VBlank flag
//! was not set at the correct PPU cycle."
//!
//! These tests verify that the CPU can observe VBlank (PPUSTATUS bit 7) on the same
//! master cycle that the PPU reaches scanline 241, dot 1 (NTSC).
//!
//! ## Boundary Read Suppression (TEST_ppu_vblank_begining_1.md)
//!
//! When CPU reads $2002 on the exact cycle that PPU would set VBlank, the VBlank
//! flag must be **suppressed** for that entire frame. This is a well-documented
//! NES hardware quirk.

use std::io::Write;
use std::path::PathBuf;
use tempfile::NamedTempFile;
use crate::apu::ApuType::RP2A03;
use crate::bus::BusType;
use crate::bus_device::BusDeviceType::{APU, CARTRIDGE, CONTROLLER, PPU, WRAM};
use crate::memory::MemoryType::StandardMemory;
use crate::cartridge::CartridgeType::NROM;
use crate::controller::ControllerType::StandardController;
use crate::cpu::CpuType;
use crate::loader::LoaderType::INESV2;
use crate::nes_console::NesConsoleBuilder;
use crate::ppu::PpuType::NES2C02;
use crate::tests::init;

/// Creates a minimal valid iNES ROM file for testing.
/// Returns the path to the temporary file.
///
/// The ROM contains:
/// - 16-byte iNES header (NROM-128: 1x16KB PRG, 1x8KB CHR)
/// - 16KB PRG ROM (filled with NOP instructions + reset vector)
/// - 8KB CHR ROM (filled with zeros)
fn create_minimal_test_rom() -> NamedTempFile {
    // iNES header for NROM-128 (mapper 0)
    let header: [u8; 16] = [
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x01,                   // 1 * 16KB PRG-ROM
        0x01,                   // 1 * 8KB CHR-ROM
        0x00,                   // Horizontal mirroring, no battery, no trainer
        0x00,                   // Mapper 0 (NROM), iNES 1.0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, // Padding
    ];

    // PRG ROM: 16KB filled with NOP ($EA) instructions
    // The CPU will execute NOPs continuously
    let mut prg_rom = vec![0xEA; 16 * 1024];

    // Set up reset vector at $FFFC-$FFFD to point to $8000 (start of PRG ROM)
    // PRG ROM is mapped at $8000-$BFFF and mirrored to $C000-$FFFF for NROM-128
    // $FFFC-$FFFD are at offset 0x3FFC-0x3FFD in our 16KB PRG ROM (mirrored)
    prg_rom[0x3FFC] = 0x00; // Low byte of $8000
    prg_rom[0x3FFD] = 0x80; // High byte of $8000

    // NMI vector at $FFFA-$FFFB - point to $8000 as well (just NOPs)
    prg_rom[0x3FFA] = 0x00;
    prg_rom[0x3FFB] = 0x80;

    // IRQ vector at $FFFE-$FFFF - point to $8000 as well
    prg_rom[0x3FFE] = 0x00;
    prg_rom[0x3FFF] = 0x80;

    // CHR ROM: 8KB filled with zeros
    let chr_rom = vec![0x00; 8 * 1024];

    // Assemble the complete ROM
    let mut rom_data = Vec::new();
    rom_data.extend_from_slice(&header);
    rom_data.extend_from_slice(&prg_rom);
    rom_data.extend_from_slice(&chr_rom);

    // Write to temp file
    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&rom_data).expect("failed to write ROM data");
    temp_file.flush().expect("failed to flush ROM data");

    temp_file
}

/// Creates a NesConsole from a ROM file path.
fn create_console(rom_path: PathBuf) -> crate::nes_console::NesConsole {
    NesConsoleBuilder::new()
        .with_cpu(CpuType::NES6502)
        .with_bus_type(BusType::NESBus)
        .with_bus_device_type(WRAM(StandardMemory))
        .with_bus_device_type(CARTRIDGE(NROM))
        .with_bus_device_type(APU(RP2A03))
        .with_bus_device_type(PPU(NES2C02))
        .with_bus_device_type(CONTROLLER(StandardController))
        .with_loader_type(INESV2)
        .with_rom_file(rom_path)
        .build()
        .expect("failed to build NesConsole")
}

/// Test that VBlank flag is set on the correct PPU dot.
///
/// On NTSC, VBlank begins at scanline 241, dot 1.
/// This test verifies that:
/// 1. Before reaching scanline 241, dot 1, VBlank flag is clear.
/// 2. On the cycle that produces scanline 241, dot 1, VBlank flag is set.
#[test]
fn test_vblank_flag_set_at_correct_cycle() {
    init();

    // Create a minimal ROM and console
    let rom_file = create_minimal_test_rom();
    let mut console = create_console(rom_file.path().to_path_buf());

    // Power on the console
    console.power_on().expect("failed to power on console");

    // NTSC timing:
    // - 262 scanlines per frame (0-261)
    // - 341 dots per scanline (0-340)
    // - VBlank starts at scanline 241, dot 1
    // - Pre-render scanline is 261

    // Track state for assertions
    let mut found_transition = false;
    let mut vblank_before_transition = false;
    let mut vblank_at_transition = false;
    let mut transition_cycle = 0u64;

    // Step through cycles until we see the VBlank transition
    // Maximum cycles to prevent infinite loop (about 2 frames worth)
    let max_cycles = 341 * 262 * 2;

    for _ in 0..max_cycles {
        let scanline_before = console.get_ppu_scanline();
        let dot_before = console.get_ppu_dot();
        let vblank_before = console.is_vblank_set();

        // Step one master cycle
        console.step_master_cycle().expect("failed to step master cycle");

        let scanline_after = console.get_ppu_scanline();
        let dot_after = console.get_ppu_dot();
        let vblank_after = console.is_vblank_set();

        // Detect the transition to scanline 241, dot 1
        // Note: dot 1 is when VBlank flag gets set (not dot 0)
        if scanline_after == 241 && dot_after == 1 {
            // Check if we just transitioned to this position
            if scanline_before != 241 || dot_before != 1 {
                found_transition = true;
                vblank_before_transition = vblank_before;
                vblank_at_transition = vblank_after;
                transition_cycle = console.get_master_cycles();
                break;
            }
        }
    }

    // Assertions
    assert!(
        found_transition,
        "Failed to find transition to scanline 241, dot 1 within {} cycles",
        max_cycles
    );

    assert!(
        !vblank_before_transition,
        "VBlank flag should be CLEAR before reaching scanline 241, dot 1 (transition at cycle {})",
        transition_cycle
    );

    assert!(
        vblank_at_transition,
        "VBlank flag should be SET at scanline 241, dot 1 (transition at cycle {})",
        transition_cycle
    );
}

/// Test that VBlank flag is cleared at the pre-render scanline.
///
/// On NTSC, VBlank ends (flag cleared) at scanline 261, dot 1.
/// This test verifies the flag is cleared at the correct time.
#[test]
fn test_vblank_flag_cleared_at_prerender_scanline() {
    init();

    let rom_file = create_minimal_test_rom();
    let mut console = create_console(rom_file.path().to_path_buf());
    console.power_on().expect("failed to power on console");

    // First, run until VBlank is set
    let max_cycles = 341 * 262 * 2;
    let mut vblank_was_set = false;

    for _ in 0..max_cycles {
        console.step_master_cycle().expect("failed to step master cycle");

        if console.is_vblank_set() {
            vblank_was_set = true;
        }

        // Once VBlank was set, look for when it gets cleared
        if vblank_was_set && !console.is_vblank_set() {
            let scanline = console.get_ppu_scanline();
            let dot = console.get_ppu_dot();

            // VBlank should be cleared at pre-render scanline (261), dot 1
            assert_eq!(
                scanline, 261,
                "VBlank flag should be cleared on pre-render scanline 261, found scanline {}",
                scanline
            );
            assert_eq!(
                dot, 1,
                "VBlank flag should be cleared at dot 1, found dot {}",
                dot
            );
            return;
        }
    }

    panic!("VBlank flag was never cleared within {} cycles", max_cycles);
}

// ============================================================================
// Boundary Read Suppression Tests (TEST_ppu_vblank_begining_1.md)
// ============================================================================

/// Creates a ROM that continuously reads $2002 (PPUSTATUS).
///
/// The ROM is filled with `LDA $2002` instructions ($AD $02 $20).
/// Each instruction takes 4 cycles:
/// - Cycle 1: Fetch opcode ($AD)
/// - Cycle 2: Fetch low byte ($02)
/// - Cycle 3: Fetch high byte ($20)
/// - Cycle 4: Read from $2002 (actual PPUSTATUS read)
///
/// This gives predictable, periodic $2002 reads every 4 CPU cycles.
fn create_status_polling_rom() -> NamedTempFile {
    // iNES header for NROM-128 (mapper 0)
    let header: [u8; 16] = [
        0x4E, 0x45, 0x53, 0x1A, // "NES\x1A" magic
        0x01,                   // 1 * 16KB PRG-ROM
        0x01,                   // 1 * 8KB CHR-ROM
        0x00,                   // Horizontal mirroring, no battery, no trainer
        0x00,                   // Mapper 0 (NROM), iNES 1.0
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
    ];

    // PRG ROM: 16KB filled with LDA $2002 instructions
    // LDA absolute = $AD, followed by address low byte, high byte
    let mut prg_rom = Vec::with_capacity(16 * 1024);

    // Fill with LDA $2002 pattern: $AD $02 $20
    while prg_rom.len() < 16 * 1024 - 6 {
        prg_rom.push(0xAD); // LDA absolute
        prg_rom.push(0x02); // Low byte of $2002
        prg_rom.push(0x20); // High byte of $2002
    }

    // Pad to vectors area
    while prg_rom.len() < 0x3FFA {
        prg_rom.push(0xEA); // NOP padding
    }

    // Set up vectors at end of PRG ROM
    // NMI vector at $FFFA-$FFFB -> $8000
    prg_rom.push(0x00); // 0x3FFA
    prg_rom.push(0x80); // 0x3FFB
    // Reset vector at $FFFC-$FFFD -> $8000
    prg_rom.push(0x00); // 0x3FFC
    prg_rom.push(0x80); // 0x3FFD
    // IRQ vector at $FFFE-$FFFF -> $8000
    prg_rom.push(0x00); // 0x3FFE
    prg_rom.push(0x80); // 0x3FFF

    // CHR ROM: 8KB filled with zeros
    let chr_rom = vec![0x00; 8 * 1024];

    // Assemble the complete ROM
    let mut rom_data = Vec::new();
    rom_data.extend_from_slice(&header);
    rom_data.extend_from_slice(&prg_rom);
    rom_data.extend_from_slice(&chr_rom);

    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&rom_data).expect("failed to write ROM data");
    temp_file.flush().expect("failed to flush ROM data");

    temp_file
}

/// Test boundary read suppression: reading $2002 on the exact cycle that would
/// set VBlank must suppress VBlank for that frame.
///
/// This test is expected to FAIL on current code because VBlank is always set
/// at scanline 241, dot 1 regardless of $2002 reads.
///
/// Per nesdev wiki and hardware tests:
/// - Reading $2002 one PPU cycle before VBlank: returns 0, VBlank sets normally
/// - Reading $2002 on the exact cycle: returns 0, VBlank is SUPPRESSED
/// - Reading $2002 one PPU cycle after: returns $80, VBlank was set
#[test]
fn test_boundary_read_suppresses_vblank() {
    init();

    let rom_file = create_status_polling_rom();
    let mut console = create_console(rom_file.path().to_path_buf());
    // Use deterministic power-on to eliminate random phase variability
    console.power_on_deterministic(0).expect("failed to power on console");

    // NTSC: 341 dots per scanline, 262 scanlines per frame
    // VBlank set at scanline 241, dot 1
    // PPU advances 3 dots per CPU cycle (master cycle)

    // We need to find a cycle where:
    // 1. CPU reads $2002 (address == 0x2002, memory_read == true)
    // 2. PPU transitions through dot 1 of scanline 241 on the same cycle

    // Search enough frames to guarantee hitting the boundary case.
    // With LDA $2002 every 4 CPU cycles and ~29781 CPU cycles per frame,
    // the read alignment shifts each frame, guaranteeing a boundary hit within 5 frames.
    let max_cycles = 341 * 262 * 5; // ~5 frames worth
    let mut found_boundary_read = false;
    let mut boundary_read_value: Option<u8> = None;
    let mut vblank_after_boundary: Option<bool> = None;

    for _ in 0..max_cycles {
        let scanline_before = console.get_ppu_scanline();
        let dot_before = console.get_ppu_dot();

        // Step one master cycle and capture the result
        let (cpu_result, _, _) = console.step_master_cycle()
            .expect("failed to step master cycle");

        let scanline_after = console.get_ppu_scanline();
        let dot_after = console.get_ppu_dot();

        // Check if CPU read $2002 on this cycle
        let cpu_read_status = cpu_result.memory_read &&
            cpu_result.address == Some(0x2002);

        // With sub-dot ordering, a master cycle processes 3 dots:
        // - advance_dots(1): 1st dot (BEFORE CPU bus op)
        // - CPU bus op
        // - advance_dots(2): 2nd and 3rd dots (AFTER CPU bus op)
        //
        // For BOUNDARY SUPPRESSION, VBlank (dot 1 of scanline 241) must fall in
        // advance_dots(2), meaning it's the 2nd or 3rd dot of this cycle.
        //
        // Detection:
        // - If dot_after == 1 or 2 on scanline 241: dot 1 was in advance_dots(2) → suppression case
        // - If dot_after == 3 on scanline 241: dot 1 was in advance_dots(1) → no suppression
        //
        // We specifically want the suppression case (dot_after == 1 or 2).
        let is_boundary_suppression_case =
            scanline_after == 241 &&
            (dot_after == 1 || dot_after == 2) &&
            (scanline_before < 241 || (scanline_before == 241 && dot_before == 0));

        if is_boundary_suppression_case && cpu_read_status {
            // Found it! CPU read $2002 during a cycle where VBlank would be set
            // AFTER the CPU read (in advance_dots(2)), triggering suppression.
            found_boundary_read = true;
            boundary_read_value = cpu_result.data;
            vblank_after_boundary = Some(console.is_vblank_set());
            break;
        }

        // If we passed the boundary without finding the suppression case, continue
        if scanline_after > 241 || (scanline_after == 0 && scanline_before > 200) {
            continue;
        }
    }

    // Assertions
    assert!(
        found_boundary_read,
        "Failed to find a cycle where CPU reads $2002 exactly on the VBlank boundary. \
         This may indicate the ROM or timing setup is incorrect."
    );

    // The read value should be 0 (VBlank not yet visible at moment of read)
    assert_eq!(
        boundary_read_value, Some(0x00),
        "Reading $2002 on the boundary cycle should return 0x00 (VBlank not visible), \
         got {:02X?}",
        boundary_read_value
    );

    // CRITICAL: VBlank should be SUPPRESSED after a boundary read
    // This is the main assertion that will FAIL on current code
    assert_eq!(
        vblank_after_boundary, Some(false),
        "VBlank should be SUPPRESSED when $2002 is read on the boundary cycle. \
         Current code incorrectly sets VBlank anyway."
    );
}

/// Test that VBlank is visible when read one cycle after the boundary.
///
/// This is the normal case - $2002 is read after VBlank has been set.
/// The read should return bit 7 = 1 and clear the flag.
#[test]
fn test_vblank_visible_after_boundary() {
    init();

    let rom_file = create_status_polling_rom();
    let mut console = create_console(rom_file.path().to_path_buf());
    // Use deterministic power-on to eliminate random phase variability
    console.power_on_deterministic(0).expect("failed to power on console");

    let max_cycles = 341 * 262 * 3;
    let mut found_post_boundary_read = false;
    let mut first_read_value: Option<u8> = None;

    for _ in 0..max_cycles {
        let scanline = console.get_ppu_scanline();
        let dot = console.get_ppu_dot();

        // Wait until we're past the VBlank boundary (scanline 241, dot > 1)
        // and VBlank should be set
        if scanline == 241 && dot > 3 && console.is_vblank_set() {
            // Now step and look for a $2002 read
            let (cpu_result, _, _) = console.step_master_cycle()
                .expect("failed to step master cycle");

            if cpu_result.memory_read && cpu_result.address == Some(0x2002) {
                found_post_boundary_read = true;
                first_read_value = cpu_result.data;
                break;
            }
        } else {
            console.step_master_cycle().expect("failed to step master cycle");
        }
    }

    assert!(
        found_post_boundary_read,
        "Failed to find a $2002 read after VBlank boundary"
    );

    // Reading $2002 after VBlank is set should return bit 7 = 1
    assert!(
        first_read_value.unwrap_or(0) & 0x80 != 0,
        "Reading $2002 after VBlank should return bit 7 set (0x80), got {:02X?}",
        first_read_value
    );

    // After the read, VBlank flag should be cleared
    assert!(
        !console.is_vblank_set(),
        "VBlank flag should be cleared after reading $2002"
    );
}
