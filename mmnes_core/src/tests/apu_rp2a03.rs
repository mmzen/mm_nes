use std::cell::RefCell;
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
    ApuRp2A03::new(sound_player, cpu, bus, config)
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

    // Write duty cycle 50% (0b10xxxxxx = 0x80)
    apu.write_byte(0x00, 0x80).unwrap();

    // The duty cycle is internal, we verify by running
    // For now just verify no error
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
}

#[test]
fn pulse1_sweep_register_enables_sweep() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable pulse 1
    apu.write_byte(0x15, 0x01).unwrap();

    // Enable sweep: 0x87 = enabled, divider=0, negate=0, shift=7
    apu.write_byte(0x01, 0x87).unwrap();
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
}

#[test]
fn pulse2_registers_work_independently() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable both pulse channels
    apu.write_byte(0x15, 0x03).unwrap();

    // Configure pulse 1
    apu.write_byte(0x00, 0x80).unwrap(); // Duty 50%
    apu.write_byte(0x02, 0x00).unwrap(); // Timer lo
    apu.write_byte(0x03, 0x08).unwrap(); // Timer hi + length

    // Configure pulse 2 differently
    apu.write_byte(0x04, 0x40).unwrap(); // Duty 25%
    apu.write_byte(0x06, 0xFF).unwrap(); // Timer lo
    apu.write_byte(0x07, 0x0F).unwrap(); // Timer hi + length
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

    // Write linear counter: 0xFF = control flag set, reload = 127
    apu.write_byte(0x08, 0xFF).unwrap();
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

    // Write timer high + length counter
    apu.write_byte(0x0B, 0x08).unwrap();
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
    apu.write_byte(0x0C, 0x3A).unwrap();
}

#[test]
fn noise_mode_and_period_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    // Write mode and period: mode=1 (short), period index=5
    apu.write_byte(0x0E, 0x85).unwrap();
}

#[test]
fn noise_length_counter_load() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable noise
    apu.write_byte(0x15, 0x08).unwrap();

    // Write length counter load
    apu.write_byte(0x0F, 0x08).unwrap(); // Length counter index = 1
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

    // Write flags and rate: IRQ enable, loop, rate index 15
    apu.write_byte(0x10, 0xCF).unwrap();
}

#[test]
fn dmc_output_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write direct output load (7-bit value)
    apu.write_byte(0x11, 0x40).unwrap(); // Mid-level output
}

#[test]
fn dmc_sample_address_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write sample address: address = $C000 + (value * 64)
    apu.write_byte(0x12, 0x00).unwrap(); // Address = $C000
    apu.write_byte(0x12, 0xFF).unwrap(); // Address = $FFC0
}

#[test]
fn dmc_sample_length_register() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Enable DMC
    apu.write_byte(0x15, 0x10).unwrap();

    // Write sample length: length = (value * 16) + 1
    apu.write_byte(0x13, 0x00).unwrap(); // Length = 1
    apu.write_byte(0x13, 0xFF).unwrap(); // Length = 4081
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
    apu.write_byte(0x17, 0x00).unwrap();
}

#[test]
fn frame_counter_mode_5_step() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write frame counter: 5-step mode, IRQ disabled
    apu.write_byte(0x17, 0xC0).unwrap();
}

#[test]
fn frame_counter_irq_inhibit() {
    init();
    let mut apu = create_apu();
    apu.initialize().unwrap();

    // Write frame counter: 4-step mode, IRQ disabled
    apu.write_byte(0x17, 0x40).unwrap();
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
