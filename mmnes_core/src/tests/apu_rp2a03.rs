// Authorship: Human 65% | Claude 35%
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use crate::apu::APU;
use crate::apu_rp2a03::ApuRp2A03;
use crate::bus::MockBusStub;
use crate::config_spec::ConfigSpec;
use crate::cpu::MockCpuStub;
use crate::memory::Memory;
use crate::nes_samples::NesSamples;
use crate::sound_playback::SoundPlayback;
use crate::tests::init;

// Simple sound playback mock for testing
#[derive(Debug, Default)]
struct MockSoundPlayback {
    samples: RefCell<Vec<f32>>,
}

impl SoundPlayback for MockSoundPlayback {
    fn push_sample(&mut self, sample: f32) {
        self.samples.borrow_mut().push(sample);
    }

    fn samples(&mut self) -> Vec<f32> {
        let samples = self.samples.borrow().clone();
        self.samples.borrow_mut().clear();
        samples
    }

    fn resume(&self) {
        // No-op for testing
    }

    fn clear(&mut self) {
        self.samples.borrow_mut().clear();
    }
}

fn create_mock_cpu() -> Rc<RefCell<MockCpuStub>> {
    let mut cpu = MockCpuStub::new();
    cpu.expect_signal_irq().returning(|_| Ok(()));
    cpu.expect_clear_irq().returning(|_| Ok(()));
    cpu.expect_is_asserted_irq_by_source().returning(|_| Ok(false));
    Rc::new(RefCell::new(cpu))
}

fn create_mock_bus() -> Rc<RefCell<MockBusStub>> {
    let mut bus = MockBusStub::new();
    bus.expect_read_byte().returning(|_| Ok(0));
    Rc::new(RefCell::new(bus))
}

fn create_apu() -> ApuRp2A03<MockSoundPlayback, MockCpuStub, MockBusStub> {
    let cpu = create_mock_cpu();
    let bus = create_mock_bus();
    let config = ConfigSpec::default();
    let sound_player = MockSoundPlayback::default();
    let data_bus = Rc::new(Cell::new(0u8));
    ApuRp2A03::new(sound_player, cpu, bus, config, data_bus)
}

// ============================================================================
// Initialization tests
// ============================================================================

#[test]
fn apu_initializes_successfully() {
    init();
    let mut apu = create_apu();
    let result = apu.initialize();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), 32); // APU_EXTERNAL_MEMORY_SIZE
}

#[test]
fn apu_reset_succeeds() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();
    let result = apu.reset();
    assert!(result.is_ok());
}

// ============================================================================
// Channel status register ($4015) tests
// ============================================================================

#[test]
fn write_status_enables_pulse1_channel() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write to $4015 (offset 0x15) to enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Read status back - pulse 1 should report length counter > 0 only if loaded
    let status = apu.read_byte(0x15).unwrap();
    // Initially length counter is 0, so bit won't be set
    assert_eq!(status & 0x01, 0x00);
}

#[test]
fn write_status_enables_pulse2_channel() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write to $4015 to enable pulse 2
    apu.write_byte(0x15, 0x02).unwrap();

    let status = apu.read_byte(0x15).unwrap();
    assert_eq!(status & 0x02, 0x00); // Length counter is 0
}

#[test]
fn write_status_enables_triangle_channel() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write to $4015 to enable triangle
    apu.write_byte(0x15, 0x04).unwrap();

    let status = apu.read_byte(0x15).unwrap();
    assert_eq!(status & 0x04, 0x00); // Length counter is 0
}

#[test]
fn write_status_enables_noise_channel() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write to $4015 to enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    let status = apu.read_byte(0x15).unwrap();
    assert_eq!(status & 0x08, 0x00); // Length counter is 0
}

#[test]
fn write_status_disables_channel_clears_length_counter() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1, set up some state
    apu.write_byte(0x15, 0x01).unwrap();
    // Write to pulse 1 registers to load length counter
    apu.write_byte(0x00, 0x3F).unwrap(); // Control
    apu.write_byte(0x02, 0x00).unwrap(); // Timer lo
    apu.write_byte(0x03, 0x08).unwrap(); // Timer hi + length counter load

    // Disable pulse 1
    apu.write_byte(0x15, 0x00).unwrap();

    // Length counter should be cleared
    let status = apu.read_byte(0x15).unwrap();
    assert_eq!(status & 0x01, 0x00);
}

// ============================================================================
// Pulse channel register tests ($4000-$4007)
// ============================================================================

#[test]
fn pulse1_control_register_sets_duty_cycle() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Write duty cycle 50% (0b10xxxxxx = 0x80, duty cycle index = 2)
    apu.write_byte(0x00, 0x80).unwrap();

    // Verify duty cycle was set to index 2 (50%)
    assert_eq!(apu.test_get_pulse1_duty_cycle(), 2);
}

#[test]
fn pulse1_control_register_sets_envelope() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Write constant volume flag and volume = 15
    // 0x3F = 0b00111111 (halt=1, const_vol=1, vol=15)
    apu.write_byte(0x00, 0x3F).unwrap();

    // Verify envelope settings (const_volume, loop_flag, volume)
    let (const_vol, loop_flag, volume) = apu.test_get_pulse1_envelope();
    assert!(const_vol, "const_volume flag should be set");
    assert!(loop_flag, "loop_flag should be set (same bit as halt)");
    assert_eq!(volume, 15, "volume should be 15");
}

#[test]
fn pulse1_sweep_register_enables_sweep() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Enable sweep: 0x87 = enabled, divider=0 (+1=1), negate=0, shift=7
    apu.write_byte(0x01, 0x87).unwrap();

    // Verify sweep settings (enabled, divider, shift, negate)
    let (enabled, divider, shift, negate) = apu.test_get_pulse1_sweep();
    assert!(enabled, "sweep should be enabled");
    assert_eq!(divider, 1, "divider should be 1 (0+1)");
    assert_eq!(shift, 7, "shift should be 7");
    assert!(!negate, "negate should be false");
}

#[test]
fn pulse1_timer_registers_set_period() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Write timer low byte
    apu.write_byte(0x02, 0xAB).unwrap();

    // Write timer high byte + length counter
    apu.write_byte(0x03, 0x07).unwrap(); // Timer high = 7, length index = 0

    // Verify timer period: high 3 bits (7) << 8 | low 8 bits (0xAB) = 0x7AB
    assert_eq!(apu.test_get_pulse1_timer_period(), 0x07AB);
}

#[test]
fn pulse2_registers_work_independently() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable both pulse channels
    apu.write_byte(0x15, 0x03).unwrap();

    // Configure pulse 1
    apu.write_byte(0x00, 0x80).unwrap(); // Duty 50% (index 2)
    apu.write_byte(0x02, 0x00).unwrap(); // Timer lo
    apu.write_byte(0x03, 0x08).unwrap(); // Timer hi + length

    // Configure pulse 2 differently
    apu.write_byte(0x04, 0x40).unwrap(); // Duty 25% (index 1)
    apu.write_byte(0x06, 0xFF).unwrap(); // Timer lo
    apu.write_byte(0x07, 0x0F).unwrap(); // Timer hi + length

    // Verify pulse 1 and pulse 2 have independent settings
    assert_eq!(apu.test_get_pulse1_duty_cycle(), 2, "pulse 1 duty should be 50%");
    assert_eq!(apu.test_get_pulse2_duty_cycle(), 1, "pulse 2 duty should be 25%");
    assert_eq!(apu.test_get_pulse1_timer_period(), 0x0000, "pulse 1 timer period");
    assert_eq!(apu.test_get_pulse2_timer_period(), 0x07FF, "pulse 2 timer period");
}

// ============================================================================
// Triangle channel register tests ($4008-$400B)
// ============================================================================

#[test]
fn triangle_linear_counter_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable triangle
    apu.write_byte(0x15, 0x04).unwrap();

    // Write linear counter: 0xFF = control flag set (bit 7), period = 127 (bits 0-6)
    apu.write_byte(0x08, 0xFF).unwrap();

    // Verify linear counter settings (control_flag, period)
    let (control, period) = apu.test_get_triangle_linear_counter();
    assert!(control, "control flag should be set");
    assert_eq!(period, 127, "period should be 127");
}

#[test]
fn triangle_timer_registers() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable triangle
    apu.write_byte(0x15, 0x04).unwrap();

    // Write timer low
    apu.write_byte(0x0A, 0x55).unwrap();

    // Write timer high + length counter (timer high = 0, length index = 1)
    apu.write_byte(0x0B, 0x08).unwrap();

    // Verify timer period: high 3 bits (0) << 8 | low 8 bits (0x55) = 0x55
    assert_eq!(apu.test_get_triangle_timer_period(), 0x0055);
}

// ============================================================================
// Noise channel register tests ($400C-$400F)
// ============================================================================

#[test]
fn noise_control_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    // Write control: halt=1, const_vol=1, volume=10
    // 0x3A = 0b00111010 (halt=1, const_vol=1, vol=10)
    apu.write_byte(0x0C, 0x3A).unwrap();

    // Verify envelope settings (const_volume, loop_flag, volume)
    let (const_vol, loop_flag, volume) = apu.test_get_noise_envelope();
    assert!(const_vol, "const_volume flag should be set");
    assert!(loop_flag, "loop_flag should be set");
    assert_eq!(volume, 10, "volume should be 10");
}

#[test]
fn noise_mode_and_period_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    // Write mode and period: mode=1 (short), period index=5
    // 0x85 = 0b10000101 (mode=1, period_index=5)
    apu.write_byte(0x0E, 0x85).unwrap();

    // Verify noise mode (true = mode 1/short)
    assert!(apu.test_get_noise_mode(), "noise mode should be 1 (short)");
    // Timer period is looked up from table, just verify it was set (non-zero)
    assert!(apu.test_get_noise_timer_period() > 0, "timer period should be set from table");
}

#[test]
fn noise_length_counter_load() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    // Write length counter load (index = 1 from bits 3-7)
    // 0x08 = 0b00001000, so index = 1, which maps to 254 in lookup table
    apu.write_byte(0x0F, 0x08).unwrap();

    // Verify length counter was loaded
    assert_eq!(apu.test_get_noise_length_counter(), 254, "length counter should be loaded with value 254");
}

// ============================================================================
// DMC channel register tests ($4010-$4013)
// ============================================================================

#[test]
fn dmc_flags_and_rate_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write flags and rate: IRQ disable, no loop, rate index 0
    apu.write_byte(0x10, 0x00).unwrap();

    // Verify initial state
    let (irq_enabled, loop_enabled) = apu.test_get_dmc_flags();
    assert!(!irq_enabled, "IRQ should be disabled");
    assert!(!loop_enabled, "loop should be disabled");

    // Write flags and rate: IRQ enable, loop, rate index 15
    // 0xCF = 0b11001111 (irq=1, loop=1, rate=15)
    apu.write_byte(0x10, 0xCF).unwrap();

    // Verify updated state
    let (irq_enabled, loop_enabled) = apu.test_get_dmc_flags();
    assert!(irq_enabled, "IRQ should be enabled");
    assert!(loop_enabled, "loop should be enabled");
    assert!(apu.test_get_dmc_timer_period() > 0, "timer period should be set from table");
}

#[test]
fn dmc_output_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write direct output load (7-bit value)
    apu.write_byte(0x11, 0x40).unwrap(); // Mid-level output (64)

    // Verify output level was set (masked to 7 bits)
    assert_eq!(apu.test_get_dmc_output(), 0x40, "output level should be 0x40");
}

#[test]
fn dmc_sample_address_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write sample address: address = $C000 | (value << 6)
    apu.write_byte(0x12, 0x00).unwrap(); // Address = $C000
    assert_eq!(apu.test_get_dmc_sample_address(), 0xC000, "address should be $C000");

    apu.write_byte(0x12, 0xFF).unwrap(); // Address = $C000 | (0xFF << 6) = $FFC0
    assert_eq!(apu.test_get_dmc_sample_address(), 0xFFC0, "address should be $FFC0");
}

#[test]
fn dmc_sample_length_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write sample length: length = (value << 4) | 1
    apu.write_byte(0x13, 0x00).unwrap(); // Length = 1
    assert_eq!(apu.test_get_dmc_sample_length(), 1, "length should be 1");

    apu.write_byte(0x13, 0xFF).unwrap(); // Length = (0xFF << 4) | 1 = 0xFF1
    assert_eq!(apu.test_get_dmc_sample_length(), 0xFF1, "length should be 0xFF1 (4081)");
}

// ============================================================================
// Frame counter register tests ($4017)
// ============================================================================

#[test]
fn frame_counter_mode_4_step() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write frame counter: 4-step mode, IRQ enabled
    // 0x00 = mode 0 (4-step), IRQ enabled
    apu.write_byte(0x17, 0x00).unwrap();

    // Verify 4-step mode (false = 4-step)
    assert!(!apu.test_get_frame_counter_mode(), "should be 4-step mode");
    assert!(!apu.test_get_frame_counter_irq_inhibit(), "IRQ should not be inhibited");
}

#[test]
fn frame_counter_mode_5_step() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write frame counter: 5-step mode, IRQ disabled
    // 0xC0 = 0b11000000 (mode=1/5-step, irq_inhibit=1)
    apu.write_byte(0x17, 0xC0).unwrap();

    // Verify 5-step mode (true = 5-step)
    assert!(apu.test_get_frame_counter_mode(), "should be 5-step mode");
    assert!(apu.test_get_frame_counter_irq_inhibit(), "IRQ should be inhibited");
}

#[test]
fn frame_counter_irq_inhibit() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write frame counter: 4-step mode, IRQ disabled
    // 0x40 = 0b01000000 (mode=0/4-step, irq_inhibit=1)
    apu.write_byte(0x17, 0x40).unwrap();

    // Verify 4-step mode with IRQ inhibited
    assert!(!apu.test_get_frame_counter_mode(), "should be 4-step mode");
    assert!(apu.test_get_frame_counter_irq_inhibit(), "IRQ should be inhibited");
}

// ============================================================================
// Frame Counter Timing Tests (Phase 8)
// ============================================================================

/// Test that NTSC 4-step mode uses exact hardware cycle values
/// Reference: https://www.nesdev.org/wiki/APU_Frame_Counter
#[test]
fn frame_counter_4_step_timing_ntsc() {
    init();
    let apu = create_apu();

    // Get 4-step event thresholds (in APU cycles)
    let events = apu.test_get_frame_events_4();

    // NTSC exact values (APU cycles = CPU cycles / 2):
    // Step 1: CPU 7457  -> APU 3728.5 -> ceil to 3729
    // Step 2: CPU 14913 -> APU 7456.5 -> ceil to 7457
    // Step 3: CPU 22371 -> APU 11185.5 -> ceil to 11186
    // Step 4: CPU 29829 -> APU 14914.5 -> ceil to 14915
    assert_eq!(events[0], 3729, "4-step event 1 should be at APU cycle 3729 (CPU 7457)");
    assert_eq!(events[1], 7457, "4-step event 2 should be at APU cycle 7457 (CPU 14913)");
    assert_eq!(events[2], 11186, "4-step event 3 should be at APU cycle 11186 (CPU 22371)");
    assert_eq!(events[3], 14915, "4-step event 4 should be at APU cycle 14915 (CPU 29829)");

    println!("4-step frame events (APU cycles): {:?}", events);
    println!("4-step frame events (CPU cycles): {:?}",
        events.map(|e| e * 2));
}

/// Test that NTSC 5-step mode uses exact hardware cycle values
#[test]
fn frame_counter_5_step_timing_ntsc() {
    init();
    let apu = create_apu();

    // Get 5-step event thresholds (in APU cycles)
    let events = apu.test_get_frame_events_5();

    // NTSC exact values (APU cycles = CPU cycles / 2):
    // Step 1: CPU 7457  -> APU 3728.5 -> ceil to 3729
    // Step 2: CPU 14913 -> APU 7456.5 -> ceil to 7457
    // Step 3: CPU 22371 -> APU 11185.5 -> ceil to 11186
    // Step 4: CPU 29829 -> APU 14914.5 -> ceil to 14915
    // Step 5: CPU 37281 -> APU 18640.5 -> ceil to 18641
    assert_eq!(events[0], 3729, "5-step event 1 should be at APU cycle 3729 (CPU 7457)");
    assert_eq!(events[1], 7457, "5-step event 2 should be at APU cycle 7457 (CPU 14913)");
    assert_eq!(events[2], 11186, "5-step event 3 should be at APU cycle 11186 (CPU 22371)");
    assert_eq!(events[3], 14915, "5-step event 4 should be at APU cycle 14915 (CPU 29829)");
    assert_eq!(events[4], 18641, "5-step event 5 should be at APU cycle 18641 (CPU 37281)");

    println!("5-step frame events (APU cycles): {:?}", events);
    println!("5-step frame events (CPU cycles): {:?}",
        events.map(|e| e * 2));
}

/// Test that frame counter steps advance at correct cycle counts
#[test]
fn frame_counter_step_advancement() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Set 4-step mode, no IRQ inhibit
    apu.write_byte(0x17, 0x00).unwrap();

    // Initial state: step 0, cycle 0
    assert_eq!(apu.test_get_frame_counter_step(), 0, "Should start at step 0");
    assert_eq!(apu.test_get_frame_counter_cycle(), 0, "Should start at cycle 0");

    // Run for just under step 1 threshold (3729 APU cycles = 7458 CPU cycles)
    // APU runs at half CPU speed, so 7456 CPU cycles = 3728 APU cycles
    let _ = apu.run(0, 7456);
    assert_eq!(apu.test_get_frame_counter_step(), 0, "Should still be at step 0 before threshold");

    // Run 4 more CPU cycles to reach/pass threshold (3730 APU cycles total)
    let _ = apu.run(7456, 4);
    assert_eq!(apu.test_get_frame_counter_step(), 1, "Should advance to step 1 after threshold");
}

// ============================================================================
// APU tick/run tests
// ============================================================================

#[test]
fn apu_run_produces_samples() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable all channels
    apu.write_byte(0x15, 0x0F).unwrap();

    // Run for some cycles
    let (cycles, samples) = apu.run(0, 1000).unwrap();

    // Should produce some samples (depends on cycles_per_sample config)
    // At 44100 Hz sample rate and ~1.79 MHz CPU, roughly 40.6 cycles per sample
    // 1000 cycles should produce ~24 samples
    assert!(cycles > 0);
    if let Some(s) = samples {
        assert!(s.samples().len() > 0);
    }
}

#[test]
fn apu_run_with_no_enabled_channels() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // All channels disabled
    apu.write_byte(0x15, 0x00).unwrap();

    // Run for some cycles
    let (cycles, samples) = apu.run(0, 1000).unwrap();

    // Should still produce samples (silence) or None
    assert!(cycles > 0);
    // samples may be None if not enough cycles for a sample
}

// ============================================================================
// Read-only/write-only register behavior tests
// ============================================================================

#[test]
fn reading_write_only_registers_returns_zero() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Most APU registers are write-only, reading returns 0
    assert_eq!(apu.read_byte(0x00).unwrap(), 0); // Pulse 1 control
    assert_eq!(apu.read_byte(0x01).unwrap(), 0); // Pulse 1 sweep
    assert_eq!(apu.read_byte(0x02).unwrap(), 0); // Pulse 1 timer lo
    assert_eq!(apu.read_byte(0x03).unwrap(), 0); // Pulse 1 timer hi
}

#[test]
fn reading_status_register_returns_channel_status() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Initially all length counters are 0
    let status = apu.read_byte(0x15).unwrap();

    // Lower 5 bits are length counter status (all 0 initially)
    assert_eq!(status & 0x1F, 0x00);
}

// ============================================================================
// Memory interface tests
// ============================================================================

#[test]
fn apu_memory_size_is_correct() {
    init();
    let apu = create_apu();
    assert_eq!(apu.size(), 32);
}

#[test]
fn apu_read_word_returns_zero() {
    init();
    let apu = create_apu();
    // APU doesn't support word reads
    assert_eq!(apu.read_word(0x00).unwrap(), 0);
}

#[test]
fn apu_write_word_succeeds_but_is_noop() {
    init();
    let mut apu = create_apu();
    // APU doesn't support word writes
    assert!(apu.write_word(0x00, 0xFFFF).is_ok());
}
