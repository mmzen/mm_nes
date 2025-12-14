//! Test functions for SingleStepTests opcode validation
//!
//! These tests load JSON test files and validate CPU behavior
//! against the SingleStepTests/65x02 test suite.

use super::runner::{run_opcode_tests, run_opcode_tests_with_limit};

/// Helper macro to generate test functions for each opcode
macro_rules! opcode_test {
    ($name:ident, $opcode:expr) => {
        #[test]
        #[ignore] // Ignored by default - run with --ignored flag
        fn $name() {
            let json_path = format!(
                "{}/../tests/data/singlestep/nes6502/{:02x}.json",
                env!("CARGO_MANIFEST_DIR"),
                $opcode
            );

            let json_content = match std::fs::read_to_string(&json_path) {
                Ok(content) => content,
                Err(e) => {
                    eprintln!("Skipping test - could not load {}: {}", json_path, e);
                    return;
                }
            };

            let (passed, failed, failures) = run_opcode_tests_with_limit(&json_content, 10);

            if !failures.is_empty() {
                println!("\nFirst {} failures for opcode 0x{:02X}:", failures.len(), $opcode);
                for result in &failures {
                    println!("{}", result);
                }
            }

            println!(
                "Opcode 0x{:02X}: {} passed, {} failed",
                $opcode, passed, failed
            );

            assert_eq!(failed, 0, "Some tests failed for opcode 0x{:02X}", $opcode);
        }
    };
}

// Generate tests for all 256 opcodes
opcode_test!(test_opcode_00, 0x00);
opcode_test!(test_opcode_01, 0x01);
opcode_test!(test_opcode_02, 0x02);
opcode_test!(test_opcode_03, 0x03);
opcode_test!(test_opcode_04, 0x04);
opcode_test!(test_opcode_05, 0x05);
opcode_test!(test_opcode_06, 0x06);
opcode_test!(test_opcode_07, 0x07);
opcode_test!(test_opcode_08, 0x08);
opcode_test!(test_opcode_09, 0x09);
opcode_test!(test_opcode_0a, 0x0A);
opcode_test!(test_opcode_0b, 0x0B);
opcode_test!(test_opcode_0c, 0x0C);
opcode_test!(test_opcode_0d, 0x0D);
opcode_test!(test_opcode_0e, 0x0E);
opcode_test!(test_opcode_0f, 0x0F);

opcode_test!(test_opcode_10, 0x10);
opcode_test!(test_opcode_11, 0x11);
opcode_test!(test_opcode_12, 0x12);
opcode_test!(test_opcode_13, 0x13);
opcode_test!(test_opcode_14, 0x14);
opcode_test!(test_opcode_15, 0x15);
opcode_test!(test_opcode_16, 0x16);
opcode_test!(test_opcode_17, 0x17);
opcode_test!(test_opcode_18, 0x18);
opcode_test!(test_opcode_19, 0x19);
opcode_test!(test_opcode_1a, 0x1A);
opcode_test!(test_opcode_1b, 0x1B);
opcode_test!(test_opcode_1c, 0x1C);
opcode_test!(test_opcode_1d, 0x1D);
opcode_test!(test_opcode_1e, 0x1E);
opcode_test!(test_opcode_1f, 0x1F);

opcode_test!(test_opcode_20, 0x20);
opcode_test!(test_opcode_21, 0x21);
opcode_test!(test_opcode_22, 0x22);
opcode_test!(test_opcode_23, 0x23);
opcode_test!(test_opcode_24, 0x24);
opcode_test!(test_opcode_25, 0x25);
opcode_test!(test_opcode_26, 0x26);
opcode_test!(test_opcode_27, 0x27);
opcode_test!(test_opcode_28, 0x28);
opcode_test!(test_opcode_29, 0x29);
opcode_test!(test_opcode_2a, 0x2A);
opcode_test!(test_opcode_2b, 0x2B);
opcode_test!(test_opcode_2c, 0x2C);
opcode_test!(test_opcode_2d, 0x2D);
opcode_test!(test_opcode_2e, 0x2E);
opcode_test!(test_opcode_2f, 0x2F);

opcode_test!(test_opcode_30, 0x30);
opcode_test!(test_opcode_31, 0x31);
opcode_test!(test_opcode_32, 0x32);
opcode_test!(test_opcode_33, 0x33);
opcode_test!(test_opcode_34, 0x34);
opcode_test!(test_opcode_35, 0x35);
opcode_test!(test_opcode_36, 0x36);
opcode_test!(test_opcode_37, 0x37);
opcode_test!(test_opcode_38, 0x38);
opcode_test!(test_opcode_39, 0x39);
opcode_test!(test_opcode_3a, 0x3A);
opcode_test!(test_opcode_3b, 0x3B);
opcode_test!(test_opcode_3c, 0x3C);
opcode_test!(test_opcode_3d, 0x3D);
opcode_test!(test_opcode_3e, 0x3E);
opcode_test!(test_opcode_3f, 0x3F);

opcode_test!(test_opcode_40, 0x40);
opcode_test!(test_opcode_41, 0x41);
opcode_test!(test_opcode_42, 0x42);
opcode_test!(test_opcode_43, 0x43);
opcode_test!(test_opcode_44, 0x44);
opcode_test!(test_opcode_45, 0x45);
opcode_test!(test_opcode_46, 0x46);
opcode_test!(test_opcode_47, 0x47);
opcode_test!(test_opcode_48, 0x48);
opcode_test!(test_opcode_49, 0x49);
opcode_test!(test_opcode_4a, 0x4A);
opcode_test!(test_opcode_4b, 0x4B);
opcode_test!(test_opcode_4c, 0x4C);
opcode_test!(test_opcode_4d, 0x4D);
opcode_test!(test_opcode_4e, 0x4E);
opcode_test!(test_opcode_4f, 0x4F);

opcode_test!(test_opcode_50, 0x50);
opcode_test!(test_opcode_51, 0x51);
opcode_test!(test_opcode_52, 0x52);
opcode_test!(test_opcode_53, 0x53);
opcode_test!(test_opcode_54, 0x54);
opcode_test!(test_opcode_55, 0x55);
opcode_test!(test_opcode_56, 0x56);
opcode_test!(test_opcode_57, 0x57);
opcode_test!(test_opcode_58, 0x58);
opcode_test!(test_opcode_59, 0x59);
opcode_test!(test_opcode_5a, 0x5A);
opcode_test!(test_opcode_5b, 0x5B);
opcode_test!(test_opcode_5c, 0x5C);
opcode_test!(test_opcode_5d, 0x5D);
opcode_test!(test_opcode_5e, 0x5E);
opcode_test!(test_opcode_5f, 0x5F);

opcode_test!(test_opcode_60, 0x60);
opcode_test!(test_opcode_61, 0x61);
opcode_test!(test_opcode_62, 0x62);
opcode_test!(test_opcode_63, 0x63);
opcode_test!(test_opcode_64, 0x64);
opcode_test!(test_opcode_65, 0x65);
opcode_test!(test_opcode_66, 0x66);
opcode_test!(test_opcode_67, 0x67);
opcode_test!(test_opcode_68, 0x68);
opcode_test!(test_opcode_69, 0x69);
opcode_test!(test_opcode_6a, 0x6A);
opcode_test!(test_opcode_6b, 0x6B);
opcode_test!(test_opcode_6c, 0x6C);
opcode_test!(test_opcode_6d, 0x6D);
opcode_test!(test_opcode_6e, 0x6E);
opcode_test!(test_opcode_6f, 0x6F);

opcode_test!(test_opcode_70, 0x70);
opcode_test!(test_opcode_71, 0x71);
opcode_test!(test_opcode_72, 0x72);
opcode_test!(test_opcode_73, 0x73);
opcode_test!(test_opcode_74, 0x74);
opcode_test!(test_opcode_75, 0x75);
opcode_test!(test_opcode_76, 0x76);
opcode_test!(test_opcode_77, 0x77);
opcode_test!(test_opcode_78, 0x78);
opcode_test!(test_opcode_79, 0x79);
opcode_test!(test_opcode_7a, 0x7A);
opcode_test!(test_opcode_7b, 0x7B);
opcode_test!(test_opcode_7c, 0x7C);
opcode_test!(test_opcode_7d, 0x7D);
opcode_test!(test_opcode_7e, 0x7E);
opcode_test!(test_opcode_7f, 0x7F);

opcode_test!(test_opcode_80, 0x80);
opcode_test!(test_opcode_81, 0x81);
opcode_test!(test_opcode_82, 0x82);
opcode_test!(test_opcode_83, 0x83);
opcode_test!(test_opcode_84, 0x84);
opcode_test!(test_opcode_85, 0x85);
opcode_test!(test_opcode_86, 0x86);
opcode_test!(test_opcode_87, 0x87);
opcode_test!(test_opcode_88, 0x88);
opcode_test!(test_opcode_89, 0x89);
opcode_test!(test_opcode_8a, 0x8A);
opcode_test!(test_opcode_8b, 0x8B);
opcode_test!(test_opcode_8c, 0x8C);
opcode_test!(test_opcode_8d, 0x8D);
opcode_test!(test_opcode_8e, 0x8E);
opcode_test!(test_opcode_8f, 0x8F);

opcode_test!(test_opcode_90, 0x90);
opcode_test!(test_opcode_91, 0x91);
opcode_test!(test_opcode_92, 0x92);
opcode_test!(test_opcode_93, 0x93);
opcode_test!(test_opcode_94, 0x94);
opcode_test!(test_opcode_95, 0x95);
opcode_test!(test_opcode_96, 0x96);
opcode_test!(test_opcode_97, 0x97);
opcode_test!(test_opcode_98, 0x98);
opcode_test!(test_opcode_99, 0x99);
opcode_test!(test_opcode_9a, 0x9A);
opcode_test!(test_opcode_9b, 0x9B);
opcode_test!(test_opcode_9c, 0x9C);
opcode_test!(test_opcode_9d, 0x9D);
opcode_test!(test_opcode_9e, 0x9E);
opcode_test!(test_opcode_9f, 0x9F);

opcode_test!(test_opcode_a0, 0xA0);
opcode_test!(test_opcode_a1, 0xA1);
opcode_test!(test_opcode_a2, 0xA2);
opcode_test!(test_opcode_a3, 0xA3);
opcode_test!(test_opcode_a4, 0xA4);
opcode_test!(test_opcode_a5, 0xA5);
opcode_test!(test_opcode_a6, 0xA6);
opcode_test!(test_opcode_a7, 0xA7);
opcode_test!(test_opcode_a8, 0xA8);
opcode_test!(test_opcode_a9, 0xA9);
opcode_test!(test_opcode_aa, 0xAA);
opcode_test!(test_opcode_ab, 0xAB);
opcode_test!(test_opcode_ac, 0xAC);
opcode_test!(test_opcode_ad, 0xAD);
opcode_test!(test_opcode_ae, 0xAE);
opcode_test!(test_opcode_af, 0xAF);

opcode_test!(test_opcode_b0, 0xB0);
opcode_test!(test_opcode_b1, 0xB1);
opcode_test!(test_opcode_b2, 0xB2);
opcode_test!(test_opcode_b3, 0xB3);
opcode_test!(test_opcode_b4, 0xB4);
opcode_test!(test_opcode_b5, 0xB5);
opcode_test!(test_opcode_b6, 0xB6);
opcode_test!(test_opcode_b7, 0xB7);
opcode_test!(test_opcode_b8, 0xB8);
opcode_test!(test_opcode_b9, 0xB9);
opcode_test!(test_opcode_ba, 0xBA);
opcode_test!(test_opcode_bb, 0xBB);
opcode_test!(test_opcode_bc, 0xBC);
opcode_test!(test_opcode_bd, 0xBD);
opcode_test!(test_opcode_be, 0xBE);
opcode_test!(test_opcode_bf, 0xBF);

opcode_test!(test_opcode_c0, 0xC0);
opcode_test!(test_opcode_c1, 0xC1);
opcode_test!(test_opcode_c2, 0xC2);
opcode_test!(test_opcode_c3, 0xC3);
opcode_test!(test_opcode_c4, 0xC4);
opcode_test!(test_opcode_c5, 0xC5);
opcode_test!(test_opcode_c6, 0xC6);
opcode_test!(test_opcode_c7, 0xC7);
opcode_test!(test_opcode_c8, 0xC8);
opcode_test!(test_opcode_c9, 0xC9);
opcode_test!(test_opcode_ca, 0xCA);
opcode_test!(test_opcode_cb, 0xCB);
opcode_test!(test_opcode_cc, 0xCC);
opcode_test!(test_opcode_cd, 0xCD);
opcode_test!(test_opcode_ce, 0xCE);
opcode_test!(test_opcode_cf, 0xCF);

opcode_test!(test_opcode_d0, 0xD0);
opcode_test!(test_opcode_d1, 0xD1);
opcode_test!(test_opcode_d2, 0xD2);
opcode_test!(test_opcode_d3, 0xD3);
opcode_test!(test_opcode_d4, 0xD4);
opcode_test!(test_opcode_d5, 0xD5);
opcode_test!(test_opcode_d6, 0xD6);
opcode_test!(test_opcode_d7, 0xD7);
opcode_test!(test_opcode_d8, 0xD8);
opcode_test!(test_opcode_d9, 0xD9);
opcode_test!(test_opcode_da, 0xDA);
opcode_test!(test_opcode_db, 0xDB);
opcode_test!(test_opcode_dc, 0xDC);
opcode_test!(test_opcode_dd, 0xDD);
opcode_test!(test_opcode_de, 0xDE);
opcode_test!(test_opcode_df, 0xDF);

opcode_test!(test_opcode_e0, 0xE0);
opcode_test!(test_opcode_e1, 0xE1);
opcode_test!(test_opcode_e2, 0xE2);
opcode_test!(test_opcode_e3, 0xE3);
opcode_test!(test_opcode_e4, 0xE4);
opcode_test!(test_opcode_e5, 0xE5);
opcode_test!(test_opcode_e6, 0xE6);
opcode_test!(test_opcode_e7, 0xE7);
opcode_test!(test_opcode_e8, 0xE8);
opcode_test!(test_opcode_e9, 0xE9);
opcode_test!(test_opcode_ea, 0xEA);
opcode_test!(test_opcode_eb, 0xEB);
opcode_test!(test_opcode_ec, 0xEC);
opcode_test!(test_opcode_ed, 0xED);
opcode_test!(test_opcode_ee, 0xEE);
opcode_test!(test_opcode_ef, 0xEF);

opcode_test!(test_opcode_f0, 0xF0);
opcode_test!(test_opcode_f1, 0xF1);
opcode_test!(test_opcode_f2, 0xF2);
opcode_test!(test_opcode_f3, 0xF3);
opcode_test!(test_opcode_f4, 0xF4);
opcode_test!(test_opcode_f5, 0xF5);
opcode_test!(test_opcode_f6, 0xF6);
opcode_test!(test_opcode_f7, 0xF7);
opcode_test!(test_opcode_f8, 0xF8);
opcode_test!(test_opcode_f9, 0xF9);
opcode_test!(test_opcode_fa, 0xFA);
opcode_test!(test_opcode_fb, 0xFB);
opcode_test!(test_opcode_fc, 0xFC);
opcode_test!(test_opcode_fd, 0xFD);
opcode_test!(test_opcode_fe, 0xFE);
opcode_test!(test_opcode_ff, 0xFF);

/// Test to run all opcodes in sequence (very long-running)
#[test]
#[ignore]
fn test_all_opcodes() {
    let mut total_passed = 0;
    let mut total_failed = 0;
    let mut failed_opcodes = Vec::new();

    for opcode in 0x00..=0xFFu8 {
        let json_path = format!(
            "{}/../tests/data/singlestep/nes6502/{:02x}.json",
            env!("CARGO_MANIFEST_DIR"),
            opcode
        );

        let json_content = match std::fs::read_to_string(&json_path) {
            Ok(content) => content,
            Err(_) => {
                println!("Skipping opcode 0x{:02X} - test file not found", opcode);
                continue;
            }
        };

        let (passed, failed, _) = run_opcode_tests(&json_content);
        total_passed += passed;
        total_failed += failed;

        if failed > 0 {
            failed_opcodes.push((opcode, failed));
        }

        println!(
            "Opcode 0x{:02X}: {} passed, {} failed",
            opcode, passed, failed
        );
    }

    println!("\n========================================");
    println!("TOTAL: {} passed, {} failed", total_passed, total_failed);

    if !failed_opcodes.is_empty() {
        println!("\nFailed opcodes:");
        for (opcode, count) in &failed_opcodes {
            println!("  0x{:02X}: {} failures", opcode, count);
        }
    }

    assert_eq!(total_failed, 0, "Some tests failed");
}

/// Quick sanity test using inline test data (doesn't require test files)
/// Note: This test validates the current CPU behavior which may differ from cycle-accurate
/// emulation. The real 6502 performs a dummy read on the next byte for 2-cycle implicit
/// mode instructions like NOP. Now cycle-accurate with dummy read.
#[test]
fn test_inline_nop_validation() {
    use super::runner::run_test_case;
    use super::{TestCase, CpuState, BusCycle, BusOperation};

    // Real 6502 NOP: [opcode fetch, dummy read of next byte]
    let test = TestCase {
        name: "ea NOP inline".to_string(),
        initial: CpuState {
            pc: 0x0200,
            s: 0xFD,
            a: 0x00,
            x: 0x00,
            y: 0x00,
            p: 0x24,
            ram: vec![(0x0200, 0xEA), (0x0201, 0x00)],
        },
        final_state: CpuState {
            pc: 0x0201,
            s: 0xFD,
            a: 0x00,
            x: 0x00,
            y: 0x00,
            p: 0x24,
            ram: vec![(0x0200, 0xEA), (0x0201, 0x00)],
        },
        // 2 cycles: opcode fetch + dummy read of next byte
        cycles: vec![
            BusCycle { address: 0x0200, value: 0xEA, operation: BusOperation::Read },
            BusCycle { address: 0x0201, value: 0x00, operation: BusOperation::Read },
        ],
    };

    let result = run_test_case(&test);

    if !result.passed {
        println!("{}", result);
    }

    assert!(result.passed, "Inline NOP test should pass (state validation only)");
}

/// Test LDA immediate instruction inline
#[test]
fn test_inline_lda_immediate() {
    use super::runner::run_test_case;
    use super::{TestCase, CpuState, BusCycle, BusOperation};

    let test = TestCase {
        name: "a9 42 LDA #$42".to_string(),
        initial: CpuState {
            pc: 0x0200,
            s: 0xFD,
            a: 0x00,
            x: 0x00,
            y: 0x00,
            p: 0x24, // IRQ disabled, Zero flag set
            ram: vec![(0x0200, 0xA9), (0x0201, 0x42)],
        },
        final_state: CpuState {
            pc: 0x0202,
            s: 0xFD,
            a: 0x42,
            x: 0x00,
            y: 0x00,
            p: 0x24, // Zero flag cleared (value is non-zero), negative flag cleared
            ram: vec![(0x0200, 0xA9), (0x0201, 0x42)],
        },
        cycles: vec![
            BusCycle { address: 0x0200, value: 0xA9, operation: BusOperation::Read },
            BusCycle { address: 0x0201, value: 0x42, operation: BusOperation::Read },
        ],
    };

    let result = run_test_case(&test);

    if !result.passed {
        println!("{}", result);
    }

    assert!(result.passed, "Inline LDA test should pass");
}
