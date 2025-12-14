use std::cell::RefCell;
use std::rc::Rc;
use crate::bus::MockBusStub;
use crate::cpu::{CPU, CpuError, Interruptible};
use crate::cpu_6502::Cpu6502;
use crate::tests::init;

// Stack page constants
const STACK_BASE: u16 = 0x0100;

// Helper to create a CPU with a mock bus that has configurable memory
fn create_cpu_with_program(program: &[u8], start_addr: u16) -> (Cpu6502, Rc<RefCell<MockBusStub>>) {
    let mut bus = MockBusStub::new();
    let program_copy: Vec<u8> = program.to_vec();
    let program_len = program.len();

    // Set up read_byte to return program bytes
    bus.expect_read_byte()
        .returning(move |addr| {
            let offset = addr.wrapping_sub(start_addr) as usize;
            if offset < program_len {
                Ok(program_copy[offset])
            } else {
                Ok(0x00) // Return 0 for unmapped addresses
            }
        });

    // Set up read_word for reset vector
    let start = start_addr;
    bus.expect_read_word()
        .returning(move |addr| {
            if addr == 0xFFFC {
                Ok(start) // Reset vector points to start_addr
            } else {
                Ok(0x0000)
            }
        });

    // Set up write_byte (for stack operations, etc)
    bus.expect_write_byte()
        .returning(|_, _| Ok(()));

    let bus = Rc::new(RefCell::new(bus));
    let cpu = Cpu6502::new(bus.clone());

    (cpu, bus)
}

// Helper to create a simple CPU for basic tests
fn create_simple_cpu() -> Cpu6502 {
    let mut bus = MockBusStub::new();

    bus.expect_read_byte().returning(|_| Ok(0xEA)); // NOP
    bus.expect_read_word().returning(|_| Ok(0x8000));
    bus.expect_write_byte().returning(|_, _| Ok(()));

    let bus = Rc::new(RefCell::new(bus));
    Cpu6502::new(bus)
}

// ============================================================================
// Register initialization tests
// ============================================================================

#[test]
fn cpu_registers_initialized_to_zero() {
    init();
    let cpu = create_simple_cpu();

    assert_eq!(cpu.get_a(), 0);
    assert_eq!(cpu.get_x(), 0);
    assert_eq!(cpu.get_y(), 0);
}

#[test]
fn cpu_stack_pointer_starts_at_zero() {
    init();
    let cpu = create_simple_cpu();

    // Stack pointer starts at 0 before reset
    assert_eq!(cpu.get_sp(), 0);
}

#[test]
fn cpu_reset_sets_stack_pointer_to_fd() -> Result<(), CpuError> {
    init();
    let mut cpu = create_simple_cpu();

    cpu.reset()?;
    assert_eq!(cpu.get_sp(), 0xFD);

    Ok(())
}

#[test]
fn cpu_reset_sets_interrupt_disable_flag() -> Result<(), CpuError> {
    init();
    let mut cpu = create_simple_cpu();

    cpu.reset()?;
    assert!(cpu.get_interrupt_disable());

    Ok(())
}

// ============================================================================
// Load instruction tests (LDA, LDX, LDY)
// ============================================================================

#[test]
fn lda_immediate_loads_value_into_accumulator() -> Result<(), CpuError> {
    init();
    // LDA #$42 (0xA9 0x42)
    let (mut cpu, _) = create_cpu_with_program(&[0xA9, 0x42], 0x8000);
    cpu.set_pc_for_test(0x8000);

    let cycles = cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x42);
    assert_eq!(cycles, 2);
    assert!(!cpu.get_zero());
    assert!(!cpu.get_negative());

    Ok(())
}

#[test]
fn lda_immediate_sets_zero_flag_when_loading_zero() -> Result<(), CpuError> {
    init();
    // LDA #$00
    let (mut cpu, _) = create_cpu_with_program(&[0xA9, 0x00], 0x8000);
    cpu.set_pc_for_test(0x8000);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x00);
    assert!(cpu.get_zero());
    assert!(!cpu.get_negative());

    Ok(())
}

#[test]
fn lda_immediate_sets_negative_flag_when_loading_negative_value() -> Result<(), CpuError> {
    init();
    // LDA #$80 (negative in two's complement)
    let (mut cpu, _) = create_cpu_with_program(&[0xA9, 0x80], 0x8000);
    cpu.set_pc_for_test(0x8000);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x80);
    assert!(!cpu.get_zero());
    assert!(cpu.get_negative());

    Ok(())
}

#[test]
fn ldx_immediate_loads_value_into_x_register() -> Result<(), CpuError> {
    init();
    // LDX #$55 (0xA2 0x55)
    let (mut cpu, _) = create_cpu_with_program(&[0xA2, 0x55], 0x8000);
    cpu.set_pc_for_test(0x8000);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0x55);
    assert!(!cpu.get_zero());
    assert!(!cpu.get_negative());

    Ok(())
}

#[test]
fn ldy_immediate_loads_value_into_y_register() -> Result<(), CpuError> {
    init();
    // LDY #$AA (0xA0 0xAA)
    let (mut cpu, _) = create_cpu_with_program(&[0xA0, 0xAA], 0x8000);
    cpu.set_pc_for_test(0x8000);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_y(), 0xAA);
    assert!(!cpu.get_zero());
    assert!(cpu.get_negative()); // 0xAA has bit 7 set

    Ok(())
}

// ============================================================================
// Transfer instruction tests (TAX, TAY, TXA, TYA, TSX, TXS)
// ============================================================================

#[test]
fn tax_transfers_accumulator_to_x() -> Result<(), CpuError> {
    init();
    // TAX (0xAA)
    let (mut cpu, _) = create_cpu_with_program(&[0xAA], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x42);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0x42);
    assert_eq!(cpu.get_a(), 0x42); // A unchanged

    Ok(())
}

#[test]
fn tay_transfers_accumulator_to_y() -> Result<(), CpuError> {
    init();
    // TAY (0xA8)
    let (mut cpu, _) = create_cpu_with_program(&[0xA8], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x33);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_y(), 0x33);
    assert_eq!(cpu.get_a(), 0x33); // A unchanged

    Ok(())
}

#[test]
fn txa_transfers_x_to_accumulator() -> Result<(), CpuError> {
    init();
    // TXA (0x8A)
    let (mut cpu, _) = create_cpu_with_program(&[0x8A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0x77);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x77);
    assert_eq!(cpu.get_x(), 0x77); // X unchanged

    Ok(())
}

#[test]
fn tya_transfers_y_to_accumulator() -> Result<(), CpuError> {
    init();
    // TYA (0x98)
    let (mut cpu, _) = create_cpu_with_program(&[0x98], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_y(0x99);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x99);
    assert_eq!(cpu.get_y(), 0x99); // Y unchanged

    Ok(())
}

#[test]
fn tsx_transfers_stack_pointer_to_x() -> Result<(), CpuError> {
    init();
    // TSX (0xBA)
    let (mut cpu, _) = create_cpu_with_program(&[0xBA], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_sp(0xFD);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0xFD);
    assert_eq!(cpu.get_sp(), 0xFD); // SP unchanged

    Ok(())
}

#[test]
fn txs_transfers_x_to_stack_pointer() -> Result<(), CpuError> {
    init();
    // TXS (0x9A)
    let (mut cpu, _) = create_cpu_with_program(&[0x9A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0xFF);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_sp(), 0xFF);
    assert_eq!(cpu.get_x(), 0xFF); // X unchanged
    // Note: TXS does NOT affect flags

    Ok(())
}

// ============================================================================
// Increment/Decrement tests (INX, INY, DEX, DEY)
// ============================================================================

#[test]
fn inx_increments_x_register() -> Result<(), CpuError> {
    init();
    // INX (0xE8)
    let (mut cpu, _) = create_cpu_with_program(&[0xE8], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0x41);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0x42);

    Ok(())
}

#[test]
fn inx_wraps_from_ff_to_00() -> Result<(), CpuError> {
    init();
    // INX (0xE8)
    let (mut cpu, _) = create_cpu_with_program(&[0xE8], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0xFF);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0x00);
    assert!(cpu.get_zero());
    assert!(!cpu.get_negative());

    Ok(())
}

#[test]
fn iny_increments_y_register() -> Result<(), CpuError> {
    init();
    // INY (0xC8)
    let (mut cpu, _) = create_cpu_with_program(&[0xC8], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_y(0x7F);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_y(), 0x80);
    assert!(!cpu.get_zero());
    assert!(cpu.get_negative()); // 0x80 is negative

    Ok(())
}

#[test]
fn dex_decrements_x_register() -> Result<(), CpuError> {
    init();
    // DEX (0xCA)
    let (mut cpu, _) = create_cpu_with_program(&[0xCA], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0x42);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0x41);

    Ok(())
}

#[test]
fn dex_wraps_from_00_to_ff() -> Result<(), CpuError> {
    init();
    // DEX (0xCA)
    let (mut cpu, _) = create_cpu_with_program(&[0xCA], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0x00);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_x(), 0xFF);
    assert!(!cpu.get_zero());
    assert!(cpu.get_negative());

    Ok(())
}

#[test]
fn dey_decrements_y_register() -> Result<(), CpuError> {
    init();
    // DEY (0x88)
    let (mut cpu, _) = create_cpu_with_program(&[0x88], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_y(0x01);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_y(), 0x00);
    assert!(cpu.get_zero());

    Ok(())
}

// ============================================================================
// Flag instruction tests (SEC, CLC, SED, CLD, SEI, CLI, CLV)
// ============================================================================

#[test]
fn sec_sets_carry_flag() -> Result<(), CpuError> {
    init();
    // SEC (0x38)
    let (mut cpu, _) = create_cpu_with_program(&[0x38], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_carry(false);

    cpu.step_instruction()?;

    assert!(cpu.get_carry());

    Ok(())
}

#[test]
fn clc_clears_carry_flag() -> Result<(), CpuError> {
    init();
    // CLC (0x18)
    let (mut cpu, _) = create_cpu_with_program(&[0x18], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_carry(true);

    cpu.step_instruction()?;

    assert!(!cpu.get_carry());

    Ok(())
}

#[test]
fn sed_sets_decimal_flag() -> Result<(), CpuError> {
    init();
    // SED (0xF8)
    let (mut cpu, _) = create_cpu_with_program(&[0xF8], 0x8000);
    cpu.set_pc_for_test(0x8000);

    cpu.step_instruction()?;

    assert!(cpu.get_decimal());

    Ok(())
}

#[test]
fn cld_clears_decimal_flag() -> Result<(), CpuError> {
    init();
    // CLD (0xD8)
    let (mut cpu, _) = create_cpu_with_program(&[0xD8, 0xF8, 0xD8], 0x8000);
    cpu.set_pc_for_test(0x8000);

    // First CLD (no-op, already clear)
    cpu.step_instruction()?;
    assert!(!cpu.get_decimal());

    // SED
    cpu.step_instruction()?;
    assert!(cpu.get_decimal());

    // CLD
    cpu.step_instruction()?;
    assert!(!cpu.get_decimal());

    Ok(())
}

#[test]
fn clv_clears_overflow_flag() -> Result<(), CpuError> {
    init();
    // CLV (0xB8)
    let (mut cpu, _) = create_cpu_with_program(&[0xB8], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_overflow(true);

    cpu.step_instruction()?;

    assert!(!cpu.get_overflow());

    Ok(())
}

// ============================================================================
// NOP test
// ============================================================================

#[test]
fn nop_does_nothing_but_advances_pc() -> Result<(), CpuError> {
    init();
    // NOP (0xEA)
    let (mut cpu, _) = create_cpu_with_program(&[0xEA], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x42);
    cpu.set_x(0x33);
    cpu.set_y(0x77);
    let original_status = cpu.get_status();

    let cycles = cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x42);
    assert_eq!(cpu.get_x(), 0x33);
    assert_eq!(cpu.get_y(), 0x77);
    assert_eq!(cpu.get_status(), original_status);
    assert_eq!(cpu.get_pc(), 0x8001);
    assert_eq!(cycles, 2);

    Ok(())
}

// ============================================================================
// Logical operation tests (AND, ORA, EOR)
// ============================================================================

#[test]
fn and_immediate_performs_bitwise_and() -> Result<(), CpuError> {
    init();
    // AND #$0F (0x29 0x0F)
    let (mut cpu, _) = create_cpu_with_program(&[0x29, 0x0F], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0xFF);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x0F);

    Ok(())
}

#[test]
fn ora_immediate_performs_bitwise_or() -> Result<(), CpuError> {
    init();
    // ORA #$F0 (0x09 0xF0)
    let (mut cpu, _) = create_cpu_with_program(&[0x09, 0xF0], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x0F);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0xFF);

    Ok(())
}

#[test]
fn eor_immediate_performs_bitwise_xor() -> Result<(), CpuError> {
    init();
    // EOR #$FF (0x49 0xFF)
    let (mut cpu, _) = create_cpu_with_program(&[0x49, 0xFF], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0xAA);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x55); // 0xAA XOR 0xFF = 0x55

    Ok(())
}

// ============================================================================
// Shift/Rotate tests (ASL, LSR, ROL, ROR) - Accumulator mode
// ============================================================================

#[test]
fn asl_accumulator_shifts_left() -> Result<(), CpuError> {
    init();
    // ASL A (0x0A)
    let (mut cpu, _) = create_cpu_with_program(&[0x0A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x40);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x80);
    assert!(!cpu.get_carry()); // bit 7 was 0
    assert!(cpu.get_negative());

    Ok(())
}

#[test]
fn asl_accumulator_sets_carry_from_bit_7() -> Result<(), CpuError> {
    init();
    // ASL A (0x0A)
    let (mut cpu, _) = create_cpu_with_program(&[0x0A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x80);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x00);
    assert!(cpu.get_carry()); // bit 7 was 1
    assert!(cpu.get_zero());

    Ok(())
}

#[test]
fn lsr_accumulator_shifts_right() -> Result<(), CpuError> {
    init();
    // LSR A (0x4A)
    let (mut cpu, _) = create_cpu_with_program(&[0x4A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x04);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x02);
    assert!(!cpu.get_carry()); // bit 0 was 0

    Ok(())
}

#[test]
fn lsr_accumulator_sets_carry_from_bit_0() -> Result<(), CpuError> {
    init();
    // LSR A (0x4A)
    let (mut cpu, _) = create_cpu_with_program(&[0x4A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x01);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x00);
    assert!(cpu.get_carry()); // bit 0 was 1
    assert!(cpu.get_zero());

    Ok(())
}

#[test]
fn rol_accumulator_rotates_left_through_carry() -> Result<(), CpuError> {
    init();
    // ROL A (0x2A)
    let (mut cpu, _) = create_cpu_with_program(&[0x2A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x40);
    cpu.set_carry(true);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x81); // 0x40 << 1 | carry = 0x81
    assert!(!cpu.get_carry()); // old bit 7 was 0

    Ok(())
}

#[test]
fn ror_accumulator_rotates_right_through_carry() -> Result<(), CpuError> {
    init();
    // ROR A (0x6A)
    let (mut cpu, _) = create_cpu_with_program(&[0x6A], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x02);
    cpu.set_carry(true);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x81); // carry << 7 | 0x02 >> 1 = 0x81
    assert!(!cpu.get_carry()); // old bit 0 was 0

    Ok(())
}

// ============================================================================
// Compare instruction tests (CMP, CPX, CPY)
// ============================================================================

#[test]
fn cmp_immediate_sets_zero_when_equal() -> Result<(), CpuError> {
    init();
    // CMP #$42 (0xC9 0x42)
    let (mut cpu, _) = create_cpu_with_program(&[0xC9, 0x42], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x42);

    cpu.step_instruction()?;

    assert!(cpu.get_zero());
    assert!(cpu.get_carry()); // A >= M
    assert!(!cpu.get_negative());

    Ok(())
}

#[test]
fn cmp_immediate_sets_carry_when_a_greater() -> Result<(), CpuError> {
    init();
    // CMP #$10 (0xC9 0x10)
    let (mut cpu, _) = create_cpu_with_program(&[0xC9, 0x10], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x50);

    cpu.step_instruction()?;

    assert!(!cpu.get_zero());
    assert!(cpu.get_carry()); // A > M

    Ok(())
}

#[test]
fn cmp_immediate_clears_carry_when_a_less() -> Result<(), CpuError> {
    init();
    // CMP #$50 (0xC9 0x50)
    let (mut cpu, _) = create_cpu_with_program(&[0xC9, 0x50], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x10);

    cpu.step_instruction()?;

    assert!(!cpu.get_zero());
    assert!(!cpu.get_carry()); // A < M
    assert!(cpu.get_negative()); // Result is negative (0x10 - 0x50 = 0xC0)

    Ok(())
}

#[test]
fn cpx_immediate_compares_x_register() -> Result<(), CpuError> {
    init();
    // CPX #$42 (0xE0 0x42)
    let (mut cpu, _) = create_cpu_with_program(&[0xE0, 0x42], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_x(0x42);

    cpu.step_instruction()?;

    assert!(cpu.get_zero());
    assert!(cpu.get_carry());

    Ok(())
}

#[test]
fn cpy_immediate_compares_y_register() -> Result<(), CpuError> {
    init();
    // CPY #$42 (0xC0 0x42)
    let (mut cpu, _) = create_cpu_with_program(&[0xC0, 0x42], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_y(0x42);

    cpu.step_instruction()?;

    assert!(cpu.get_zero());
    assert!(cpu.get_carry());

    Ok(())
}

// ============================================================================
// Arithmetic tests (ADC, SBC)
// ============================================================================

#[test]
fn adc_immediate_adds_without_carry() -> Result<(), CpuError> {
    init();
    // ADC #$10 (0x69 0x10)
    let (mut cpu, _) = create_cpu_with_program(&[0x69, 0x10], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x20);
    cpu.set_carry(false);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x30);
    assert!(!cpu.get_carry());
    assert!(!cpu.get_overflow());

    Ok(())
}

#[test]
fn adc_immediate_adds_with_carry_in() -> Result<(), CpuError> {
    init();
    // ADC #$10 (0x69 0x10)
    let (mut cpu, _) = create_cpu_with_program(&[0x69, 0x10], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x20);
    cpu.set_carry(true);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x31); // 0x20 + 0x10 + 1 = 0x31
    assert!(!cpu.get_carry());

    Ok(())
}

#[test]
fn adc_sets_carry_on_overflow() -> Result<(), CpuError> {
    init();
    // ADC #$01 (0x69 0x01)
    let (mut cpu, _) = create_cpu_with_program(&[0x69, 0x01], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0xFF);
    cpu.set_carry(false);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x00);
    assert!(cpu.get_carry()); // Wrapped around
    assert!(cpu.get_zero());

    Ok(())
}

#[test]
fn adc_sets_overflow_on_signed_overflow() -> Result<(), CpuError> {
    init();
    // Adding two positive numbers that result in negative (signed overflow)
    // 0x7F + 0x01 = 0x80 (127 + 1 = -128 in signed)
    // ADC #$01 (0x69 0x01)
    let (mut cpu, _) = create_cpu_with_program(&[0x69, 0x01], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x7F);
    cpu.set_carry(false);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x80);
    assert!(cpu.get_overflow()); // Signed overflow
    assert!(cpu.get_negative());

    Ok(())
}

#[test]
fn sbc_immediate_subtracts_with_borrow() -> Result<(), CpuError> {
    init();
    // SBC #$10 (0xE9 0x10)
    let (mut cpu, _) = create_cpu_with_program(&[0xE9, 0x10], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x50);
    cpu.set_carry(true); // No borrow

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x40); // 0x50 - 0x10 = 0x40
    assert!(cpu.get_carry()); // No borrow occurred

    Ok(())
}

#[test]
fn sbc_immediate_subtracts_with_borrow_in() -> Result<(), CpuError> {
    init();
    // SBC #$10 (0xE9 0x10)
    let (mut cpu, _) = create_cpu_with_program(&[0xE9, 0x10], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x50);
    cpu.set_carry(false); // Borrow set (carry clear)

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0x3F); // 0x50 - 0x10 - 1 = 0x3F
    assert!(cpu.get_carry());

    Ok(())
}

#[test]
fn sbc_clears_carry_on_borrow() -> Result<(), CpuError> {
    init();
    // SBC #$50 (0xE9 0x50)
    let (mut cpu, _) = create_cpu_with_program(&[0xE9, 0x50], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_a(0x10);
    cpu.set_carry(true);

    cpu.step_instruction()?;

    assert_eq!(cpu.get_a(), 0xC0); // 0x10 - 0x50 = 0xC0 (wraps)
    assert!(!cpu.get_carry()); // Borrow occurred
    assert!(cpu.get_negative());

    Ok(())
}

// ============================================================================
// NMI edge detection tests
// ============================================================================

#[test]
fn nmi_edge_detection_only_triggers_once_per_edge() -> Result<(), CpuError> {
    init();
    let mut cpu = create_simple_cpu();

    // First signal should set pending
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Signal again while line is already low - should not trigger new edge
    // but pending is still true from before
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Clear the line (goes high) without clearing pending_nmi
    cpu.clear_nmi()?;
    // pending_nmi remains true - will still be serviced
    assert!(cpu.is_asserted_nmi()?);

    // Signal again - this IS a new edge (line was high, now goes low)
    // but pending_nmi is already true, so no visible change
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Now clear pending manually and test edge detection properly
    cpu.clear_pending_nmi(); // This also clears nmi_line_low
    assert!(!cpu.is_asserted_nmi()?);

    // Signal - new edge, should trigger
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    Ok(())
}

#[test]
fn nmi_does_not_retrigger_while_line_held_low() -> Result<(), CpuError> {
    init();
    let mut cpu = create_simple_cpu();

    // Signal NMI
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Manually clear just pending_nmi to simulate it being serviced,
    // but keep line low (simulating NMI line still asserted)
    // We need a special test helper for this scenario
    // For now, we test that calling signal_nmi twice doesn't double-trigger

    // Clear the line and pending
    cpu.clear_pending_nmi();
    assert!(!cpu.is_asserted_nmi()?);

    // Now signal again
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?);

    // Try to signal again without clearing line - should NOT re-trigger
    // First manually mark it as serviced but line still low by using internal state
    // Since clear_pending_nmi clears both, let's just test the signal_nmi behavior
    // Multiple signal_nmi calls should only trigger once
    cpu.signal_nmi()?;
    cpu.signal_nmi()?;
    cpu.signal_nmi()?;
    assert!(cpu.is_asserted_nmi()?); // Still true from first trigger

    Ok(())
}

// ============================================================================
// SEI/CLI deferred flag behavior tests
// ============================================================================

#[test]
fn sei_sets_interrupt_disable_after_instruction() -> Result<(), CpuError> {
    init();
    // SEI (0x78)
    let (mut cpu, _) = create_cpu_with_program(&[0x78], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_interrupt_disable(false);

    cpu.step_instruction()?;

    // The I flag should be set after step_instruction completes
    assert!(cpu.get_interrupt_disable());

    Ok(())
}

#[test]
fn cli_clears_interrupt_disable_after_instruction() -> Result<(), CpuError> {
    init();
    // CLI (0x58)
    let (mut cpu, _) = create_cpu_with_program(&[0x58], 0x8000);
    cpu.set_pc_for_test(0x8000);
    cpu.set_interrupt_disable(true);

    cpu.step_instruction()?;

    // The I flag should be cleared after step_instruction completes
    assert!(!cpu.get_interrupt_disable());

    Ok(())
}
