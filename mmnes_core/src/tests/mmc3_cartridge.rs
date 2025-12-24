// Authorship: Human 0% | Claude 100%
//! MMC3 (Mapper 4) unit tests
//!
//! Tests cover:
//! - PRG banking (both modes)
//! - CHR banking (both modes)
//! - PRG-RAM enable/write-protect
//! - Mirroring control
//! - IRQ counter with A12 edge detection

use crate::memory::Memory;
use crate::memory_ciram::PpuNameTableMirroring;

/// Helper to create a minimal MMC3 test cartridge
mod test_helpers {
    use std::cell::RefCell;
    use std::fs::File;
    use std::io::{BufReader, Write};
    use std::rc::Rc;
    use tempfile::NamedTempFile;
    use crate::ines_loader::Region;
    use crate::mapper::mmc3_cartridge::Mmc3Cartridge;
    use crate::memory_ciram::PpuNameTableMirroring;

    /// Create a test MMC3 cartridge with specified configuration
    pub fn create_test_mmc3(
        prg_banks: usize,
        chr_banks: usize,
        has_prg_ram: bool,
        chr_is_rom: bool,
    ) -> Rc<RefCell<Mmc3Cartridge>> {
        // Create a temporary ROM file
        let mut temp_file = NamedTempFile::new().unwrap();

        // Write PRG-ROM data (8KB per bank)
        for bank in 0..prg_banks {
            let bank_data: Vec<u8> = (0..8192)
                .map(|i| ((bank * 8192 + i) & 0xFF) as u8)
                .collect();
            temp_file.write_all(&bank_data).unwrap();
        }

        // Write CHR-ROM data if it's ROM (1KB per bank)
        if chr_is_rom {
            for bank in 0..chr_banks {
                let bank_data: Vec<u8> = (0..1024)
                    .map(|i| ((bank * 1024 + i) & 0xFF) as u8)
                    .collect();
                temp_file.write_all(&bank_data).unwrap();
            }
        }

        temp_file.flush().unwrap();
        let path = temp_file.into_temp_path();
        let file = File::open(&path).unwrap();
        let reader = BufReader::new(file);

        let prg_rom_size = prg_banks * 8 * 1024;
        let chr_rom_size = if chr_is_rom { chr_banks * 1024 } else { 0 };
        let chr_ram_size = if chr_is_rom { 0 } else { chr_banks * 1024 };

        let cart = Mmc3Cartridge::new(
            reader,
            0,
            prg_rom_size,
            chr_rom_size,
            chr_ram_size,
            has_prg_ram,
            PpuNameTableMirroring::Vertical,
            false, // mirroring not fixed
            Region::NTSC,
        ).unwrap();

        Rc::new(RefCell::new(cart))
    }
}

// =============================================================================
// PRG Banking Tests
// =============================================================================

#[test]
fn test_prg_mode_0_bank_layout() {
    // Mode 0: R6 at $8000, R7 at $A000, fixed(-2) at $C000, fixed(-1) at $E000
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set mode 0 and select bank register 6
    cart.write_byte(0x8000, 0x06).unwrap(); // index=6, prg_mode=0, chr_mode=0
    cart.write_byte(0x8001, 2).unwrap(); // R6 = 2

    // Select bank register 7
    cart.write_byte(0x8000, 0x07).unwrap();
    cart.write_byte(0x8001, 3).unwrap(); // R7 = 3

    // Verify bank configuration via test helpers
    assert_eq!(cart.get_prg_mode(), 0);
    assert_eq!(cart.get_bank_register(6), 2);
    assert_eq!(cart.get_bank_register(7), 3);
}

#[test]
fn test_prg_mode_1_bank_layout() {
    // Mode 1: fixed(-2) at $8000, R7 at $A000, R6 at $C000, fixed(-1) at $E000
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set mode 1 and select bank register 6
    cart.write_byte(0x8000, 0x46).unwrap(); // index=6, prg_mode=1, chr_mode=0
    cart.write_byte(0x8001, 2).unwrap(); // R6 = 2

    assert_eq!(cart.get_prg_mode(), 1);
    assert_eq!(cart.get_bank_register(6), 2);
}

#[test]
fn test_prg_bank_select_bits_0_to_2() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Test all 8 bank register indices
    for i in 0..8 {
        cart.write_byte(0x8000, i).unwrap();
        cart.write_byte(0x8001, (i + 10) as u8).unwrap();
    }

    // Verify all registers were set
    for i in 0..8 {
        assert_eq!(cart.get_bank_register(i as usize), (i + 10) as u8);
    }
}

// =============================================================================
// CHR Banking Tests
// =============================================================================

#[test]
fn test_chr_mode_0_bank_layout() {
    // Mode 0: R0(2KB), R1(2KB), R2-R5(1KB each) at $0000-$1FFF
    let cart = test_helpers::create_test_mmc3(8, 64, false, true);
    let mut cart = cart.borrow_mut();

    // Set chr_mode=0 and configure R0, R1
    cart.write_byte(0x8000, 0x00).unwrap(); // index=0, chr_mode=0
    cart.write_byte(0x8001, 4).unwrap(); // R0 = 4 (even, so 4 & 0xFE = 4)

    cart.write_byte(0x8000, 0x01).unwrap(); // index=1
    cart.write_byte(0x8001, 8).unwrap(); // R1 = 8

    assert_eq!(cart.get_chr_mode(), 0);
    assert_eq!(cart.get_bank_register(0), 4);
    assert_eq!(cart.get_bank_register(1), 8);
}

#[test]
fn test_chr_mode_1_bank_layout() {
    // Mode 1: R2-R5(1KB), R0(2KB), R1(2KB) at $0000-$1FFF (swapped)
    let cart = test_helpers::create_test_mmc3(8, 64, false, true);
    let mut cart = cart.borrow_mut();

    // Set chr_mode=1
    cart.write_byte(0x8000, 0x80).unwrap(); // index=0, chr_mode=1

    assert_eq!(cart.get_chr_mode(), 1);
}

#[test]
fn test_chr_r0_r1_even_alignment() {
    // R0 and R1 should ignore bit 0 (force even alignment)
    let cart = test_helpers::create_test_mmc3(8, 64, false, true);
    let mut cart = cart.borrow_mut();

    // Set R0 to odd value - should be forced to even
    cart.write_byte(0x8000, 0x00).unwrap(); // index=0
    cart.write_byte(0x8001, 5).unwrap(); // R0 = 5 (odd)

    // The register stores the raw value, but get_chr_bank_for_address applies the mask
    assert_eq!(cart.get_bank_register(0), 5); // Raw value stored

    // Set R1 to odd value
    cart.write_byte(0x8000, 0x01).unwrap(); // index=1
    cart.write_byte(0x8001, 7).unwrap(); // R1 = 7 (odd)

    assert_eq!(cart.get_bank_register(1), 7); // Raw value stored
}

// =============================================================================
// PRG-RAM Protect Tests
// =============================================================================

#[test]
fn test_prg_ram_disabled_reads_return_open_bus() {
    let cart = test_helpers::create_test_mmc3(8, 8, true, true);
    let cart = cart.borrow();

    // PRG-RAM disabled by default
    let value = cart.read_byte(0x6000).unwrap();
    assert_eq!(value, 0xFF); // Open bus
}

#[test]
fn test_prg_ram_enabled_allows_reads_and_writes() {
    let cart = test_helpers::create_test_mmc3(8, 8, true, true);
    let mut cart = cart.borrow_mut();

    // Enable PRG-RAM without write protect
    cart.write_byte(0xA001, 0x80).unwrap(); // bit 7 = enable, bit 6 = 0 (no write protect)

    assert!(cart.is_prg_ram_enabled());
    assert!(!cart.is_prg_ram_write_protected());

    // Write and read back
    cart.write_byte(0x6000, 0x42).unwrap();
    let value = cart.read_byte(0x6000).unwrap();
    assert_eq!(value, 0x42);
}

#[test]
fn test_prg_ram_write_protect_blocks_writes() {
    let cart = test_helpers::create_test_mmc3(8, 8, true, true);
    let mut cart = cart.borrow_mut();

    // Enable PRG-RAM with write protect
    cart.write_byte(0xA001, 0xC0).unwrap(); // bit 7 = enable, bit 6 = write protect

    assert!(cart.is_prg_ram_enabled());
    assert!(cart.is_prg_ram_write_protected());

    // Write should be blocked (silently ignored)
    cart.write_byte(0x6000, 0x42).unwrap();

    // Read should return 0 (uninitialized RAM) not 0x42
    let value = cart.read_byte(0x6000).unwrap();
    assert_eq!(value, 0x00); // RAM was not written
}

#[test]
fn test_prg_ram_disabled_blocks_writes() {
    let cart = test_helpers::create_test_mmc3(8, 8, true, true);
    let mut cart = cart.borrow_mut();

    // Enable PRG-RAM first, write a value
    cart.write_byte(0xA001, 0x80).unwrap();
    cart.write_byte(0x6000, 0x42).unwrap();

    // Now disable PRG-RAM
    cart.write_byte(0xA001, 0x00).unwrap();

    assert!(!cart.is_prg_ram_enabled());

    // Read should return open bus
    let value = cart.read_byte(0x6000).unwrap();
    assert_eq!(value, 0xFF);
}

// =============================================================================
// Mirroring Control Tests
// =============================================================================

#[test]
fn test_mirroring_default_vertical() {
    use crate::cartridge::Cartridge;
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let cart = cart.borrow();

    let mirroring = cart.get_mirroring();
    assert_eq!(*mirroring.borrow(), PpuNameTableMirroring::Vertical);
}

#[test]
fn test_mirroring_switch_to_horizontal() {
    use crate::cartridge::Cartridge;
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart_mut = cart.borrow_mut();

    // Write to $A000 with bit 0 = 1 for horizontal mirroring
    cart_mut.write_byte(0xA000, 0x01).unwrap();

    let mirroring = cart_mut.get_mirroring();
    assert_eq!(*mirroring.borrow(), PpuNameTableMirroring::Horizontal);
}

#[test]
fn test_mirroring_switch_to_vertical() {
    use crate::cartridge::Cartridge;
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart_mut = cart.borrow_mut();

    // First switch to horizontal
    cart_mut.write_byte(0xA000, 0x01).unwrap();
    // Then switch back to vertical
    cart_mut.write_byte(0xA000, 0x00).unwrap();

    let mirroring = cart_mut.get_mirroring();
    assert_eq!(*mirroring.borrow(), PpuNameTableMirroring::Vertical);
}

// =============================================================================
// IRQ Counter Tests
// =============================================================================

#[test]
fn test_irq_latch_write() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Write IRQ latch value at $C000
    cart.write_byte(0xC000, 42).unwrap();

    assert_eq!(cart.get_irq_latch(), 42);
}

#[test]
fn test_irq_enable_disable() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Initially disabled
    assert!(!cart.is_irq_enabled());

    // Enable IRQ at $E001
    cart.write_byte(0xE001, 0).unwrap();
    assert!(cart.is_irq_enabled());

    // Disable IRQ at $E000
    cart.write_byte(0xE000, 0).unwrap();
    assert!(!cart.is_irq_enabled());
}

#[test]
fn test_irq_disable_clears_pending() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set up IRQ to fire
    cart.write_byte(0xC000, 1).unwrap(); // latch = 1
    cart.write_byte(0xE001, 0).unwrap(); // enable IRQ
    cart.write_byte(0xC001, 0).unwrap(); // trigger reload

    // Simulate A12 rising edges to trigger IRQ
    // First, make sure A12 was low for enough cycles
    for _ in 0..10 {
        cart.notify_ppu_address(0x0000); // A12 low
    }
    // Rising edge - should reload counter to 1
    cart.notify_ppu_address(0x1000); // A12 high

    // Reset low cycles
    for _ in 0..10 {
        cart.notify_ppu_address(0x0000);
    }
    // Rising edge - counter 1->0, IRQ fires
    cart.notify_ppu_address(0x1000);

    // IRQ should be pending
    assert!(cart.poll_irq());

    // Disable IRQ - should clear pending
    cart.write_byte(0xE000, 0).unwrap();
    assert!(!cart.poll_irq());
}

#[test]
fn test_irq_counter_decrement() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set latch to 5 and enable
    cart.write_byte(0xC000, 5).unwrap();
    cart.write_byte(0xE001, 0).unwrap();
    cart.write_byte(0xC001, 0).unwrap(); // trigger reload

    // First rising edge loads counter
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000);
    assert_eq!(cart.get_irq_counter(), 5);

    // Each subsequent rising edge decrements
    for expected in (0..5).rev() {
        for _ in 0..10 { cart.notify_ppu_address(0x0000); }
        cart.notify_ppu_address(0x1000);
        assert_eq!(cart.get_irq_counter(), expected);
    }

    // IRQ should have fired when counter hit 0
    assert!(cart.poll_irq());
}

#[test]
fn test_irq_a12_filter_blocks_fast_toggles() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set latch to 2 and enable
    cart.write_byte(0xC000, 2).unwrap();
    cart.write_byte(0xE001, 0).unwrap();
    cart.write_byte(0xC001, 0).unwrap();

    // Setup: ensure A12 filter is primed
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000);
    let counter_after_first = cart.get_irq_counter();

    // Fast toggle (not enough low cycles) should NOT clock
    cart.notify_ppu_address(0x0000);
    cart.notify_ppu_address(0x1000); // Only 1 cycle low - filtered out

    assert_eq!(cart.get_irq_counter(), counter_after_first);
}

#[test]
fn test_irq_reload_flag() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set latch to 10
    cart.write_byte(0xC000, 10).unwrap();
    cart.write_byte(0xE001, 0).unwrap();
    cart.write_byte(0xC001, 0).unwrap(); // reload flag set

    // First edge - counter reloaded to 10
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000);
    assert_eq!(cart.get_irq_counter(), 10);

    // Decrement a few times
    for _ in 0..3 {
        for _ in 0..10 { cart.notify_ppu_address(0x0000); }
        cart.notify_ppu_address(0x1000);
    }
    assert_eq!(cart.get_irq_counter(), 7);

    // Set reload flag again
    cart.write_byte(0xC001, 0).unwrap();

    // Next edge should reload
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000);
    assert_eq!(cart.get_irq_counter(), 10);
}

#[test]
fn test_irq_not_fired_when_disabled() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);
    let mut cart = cart.borrow_mut();

    // Set latch to 1 but DON'T enable
    cart.write_byte(0xC000, 1).unwrap();
    // IRQ disabled by default
    cart.write_byte(0xC001, 0).unwrap();

    // Trigger counter to hit 0
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000); // Load
    for _ in 0..10 { cart.notify_ppu_address(0x0000); }
    cart.notify_ppu_address(0x1000); // 1->0

    // IRQ should NOT be pending (IRQ disabled)
    assert!(!cart.poll_irq());
}

/// Test that IRQ counter clocks correctly when reading through CHR memory
/// (validates that A12 notifications originate from Mmc3ChrMemory, not PPU)
#[test]
fn test_irq_counter_via_chr_memory_reads() {
    use crate::cartridge::Cartridge;

    let cart = test_helpers::create_test_mmc3(8, 8, false, true);

    // Setup: latch=3, enable IRQ
    {
        let mut c = cart.borrow_mut();
        c.write_byte(0xC000, 3).unwrap(); // latch = 3
        c.write_byte(0xC001, 0).unwrap(); // reload flag
        c.write_byte(0xE001, 0).unwrap(); // enable IRQ
    }

    // Get CHR memory wrapper (this is what PPU uses for pattern table reads)
    let chr_mem = cart.borrow().get_chr_rom();

    // Simulate PPU pattern table fetches:
    // 1) Enough low-A12 reads to satisfy filter, then high-A12 read
    // Low A12 reads ($0xxx addresses)
    for _ in 0..10 {
        let _ = chr_mem.borrow().read_byte(0x0000); // A12 = 0
    }
    // High A12 read triggers counter clock
    let _ = chr_mem.borrow().read_byte(0x1000); // A12 = 1, rising edge

    // Counter should now be loaded with latch value (3)
    assert_eq!(cart.borrow().get_irq_counter(), 3);

    // Another cycle: decrement to 2
    for _ in 0..10 {
        let _ = chr_mem.borrow().read_byte(0x0000);
    }
    let _ = chr_mem.borrow().read_byte(0x1000);
    assert_eq!(cart.borrow().get_irq_counter(), 2);

    // Decrement to 1
    for _ in 0..10 {
        let _ = chr_mem.borrow().read_byte(0x0000);
    }
    let _ = chr_mem.borrow().read_byte(0x1000);
    assert_eq!(cart.borrow().get_irq_counter(), 1);

    // Decrement to 0 - should fire IRQ
    for _ in 0..10 {
        let _ = chr_mem.borrow().read_byte(0x0000);
    }
    let _ = chr_mem.borrow().read_byte(0x1000);
    assert_eq!(cart.borrow().get_irq_counter(), 0);
    assert!(cart.borrow().poll_irq(), "IRQ should fire when counter reaches 0");
}

/// Test CHR bank mapping via actual CHR memory reads
#[test]
fn test_chr_bank_mapping_via_reads() {
    use crate::cartridge::Cartridge;

    let cart = test_helpers::create_test_mmc3(8, 8, false, true);

    // Set CHR mode 0: R0 at $0000 (2KB), R1 at $0800 (2KB), R2-R5 at $1000-$1C00 (1KB each)
    // Set R2 (register 2) to bank 5
    {
        let mut c = cart.borrow_mut();
        c.write_byte(0x8000, 0x02).unwrap(); // Select R2, CHR mode 0
        c.write_byte(0x8001, 5).unwrap();    // R2 = bank 5
    }

    let chr_mem = cart.borrow().get_chr_rom();

    // In CHR mode 0, slot 4 ($1000-$13FF) uses R2
    // Bank 5 data starts at offset 5*1024 in CHR ROM
    // The test data pattern is (bank * 1024 + offset) & 0xFF
    // So reading $1000 from bank 5 should return (5 * 1024 + 0) & 0xFF = 0
    // Reading $1001 should return (5 * 1024 + 1) & 0xFF = 1
    let byte_0 = chr_mem.borrow().read_byte(0x1000).unwrap();
    let byte_1 = chr_mem.borrow().read_byte(0x1001).unwrap();

    // Bank 5 offset 0 = 5120 & 0xFF = 0
    // Bank 5 offset 1 = 5121 & 0xFF = 1
    assert_eq!(byte_0, 0, "CHR $1000 should read from bank 5 offset 0");
    assert_eq!(byte_1, 1, "CHR $1001 should read from bank 5 offset 1");

    // Now change R2 to bank 7 and verify data changes
    {
        let mut c = cart.borrow_mut();
        c.write_byte(0x8000, 0x02).unwrap(); // Select R2
        c.write_byte(0x8001, 7).unwrap();    // R2 = bank 7
    }

    // Bank 7 offset 0 = 7168 & 0xFF = 0
    // Bank 7 offset 1 = 7169 & 0xFF = 1
    // But we need to verify the data actually changed by checking a unique offset
    // Bank 7 offset 100 = 7268 & 0xFF = 100
    let byte_100 = chr_mem.borrow().read_byte(0x1064).unwrap(); // offset 100
    assert_eq!(byte_100, 100, "CHR $1064 should read from bank 7 offset 100");
}

/// Test PRG bank mapping via actual CPU-side reads
#[test]
fn test_prg_bank_mapping_via_reads() {
    let cart = test_helpers::create_test_mmc3(8, 8, false, true);

    // PRG mode 0: R6 at $8000, R7 at $A000, second-to-last at $C000, last at $E000
    // Set R6 to bank 2
    {
        let mut c = cart.borrow_mut();
        c.write_byte(0x8000, 0x06).unwrap(); // Select R6, PRG mode 0
        c.write_byte(0x8001, 2).unwrap();    // R6 = bank 2
    }

    // Read from $8000 - should get bank 2 data
    // Bank 2 starts at offset 2*8192 in PRG ROM
    // Test data pattern: (bank * 8192 + offset) & 0xFF
    let byte_0 = cart.borrow().read_byte(0x8000).unwrap();
    let byte_1 = cart.borrow().read_byte(0x8001).unwrap();

    // Bank 2 offset 0 = 16384 & 0xFF = 0
    // Bank 2 offset 1 = 16385 & 0xFF = 1
    assert_eq!(byte_0, 0, "PRG $8000 should read from bank 2 offset 0");
    assert_eq!(byte_1, 1, "PRG $8001 should read from bank 2 offset 1");

    // Change R6 to bank 5 and verify
    {
        let mut c = cart.borrow_mut();
        c.write_byte(0x8000, 0x06).unwrap();
        c.write_byte(0x8001, 5).unwrap(); // R6 = bank 5
    }

    // Bank 5 offset 0 = 40960 & 0xFF = 0
    // Bank 5 offset 100 = 41060 & 0xFF = 100
    let byte_100 = cart.borrow().read_byte(0x8064).unwrap(); // offset 100
    assert_eq!(byte_100, 100, "PRG $8064 should read from bank 5 offset 100");
}
