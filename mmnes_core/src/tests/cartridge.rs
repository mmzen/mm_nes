use std::io::{BufReader, Seek, SeekFrom, Write};
use tempfile::NamedTempFile;
use crate::bus_device::BusDevice;
use crate::cartridge::{create_chr_ram_memory, create_chr_rom_memory, create_split_ram_memory, create_split_rom_memory, get_chr_memory_size_and_type, get_first_bank_or_fail, write_rom_data};
use crate::memory::Memory;
use crate::memory_bank::MemoryBank;
use crate::tests::init;

const CPU_ADDRESS_SPACE: (u16, u16) = (0x8000, 0xFFFF);

#[test]
fn write_rom_data_successfully_writes_valid_buffer_to_memory() {
    init();

    let test_data = vec![0xAB, 0xCD, 0xEF, 0x12];
    let size = test_data.len();

    // Create a temporary file with test data
    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let mut buf_reader = BufReader::new(file);

    // Create a memory bank to write to
    let mut memory_bank = MemoryBank::new(size, CPU_ADDRESS_SPACE);
    memory_bank.initialize().expect("failed to initialize memory bank");

    // Call the function under test
    let result = write_rom_data(&mut memory_bank, size, &mut buf_reader);

    // Verify the operation succeeded
    assert_eq!(result, Ok(()));

    // Verify the data was written correctly
    for (i, &expected_byte) in test_data.iter().enumerate() {
        let actual_byte = memory_bank.read_byte(i as u16).expect("failed to read byte");
        assert_eq!(actual_byte, expected_byte);
    }
}

#[test]
fn get_chr_memory_size_and_type_returns_chr_rom_size_and_true_when_chr_rom_size_is_greater_than_zero() {
    init();

    let chr_rom_size = 8192;
    let chr_ram_size = 0;

    let result = get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);

    assert_eq!(result, (chr_rom_size, true));
}

#[test]
fn get_chr_memory_size_and_type_returns_chr_ram_size_and_false_when_chr_rom_size_is_zero() {
    init();

    let chr_rom_size = 0;
    let chr_ram_size = 4096;

    let result = get_chr_memory_size_and_type(chr_rom_size, chr_ram_size);

    assert_eq!(result, (chr_ram_size, false));
}

#[test]
fn create_split_ram_memory_creates_correct_number_of_banks_and_address_ranges() {
    init();

    let total_size = 8192; // 8KB
    let bank_size = 2048;  // 2KB
    let expected_num_banks = 4; // 4 banks

    let result = create_split_ram_memory(total_size, bank_size, CPU_ADDRESS_SPACE);

    assert!(result.is_ok());
    let memory_banks  = result.unwrap();
    let num_banks = memory_banks.len();

    assert_eq!(num_banks, expected_num_banks);

    for bank in memory_banks {
        assert_eq!(bank.size(), bank_size);
        assert_eq!(bank.get_virtual_address_range(), CPU_ADDRESS_SPACE);
    }
}

#[test]
fn create_split_rom_memory_creates_correct_number_of_banks_from_file_at_offset() {
    init();

    let test_data = vec![0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A];
    let offset = 4u64;
    let total_size = 4;
    let bank_size = 2;
    let expected_num_banks = 2;

    // Create a temporary file with test data
    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let mut buf_reader = BufReader::new(file);

    // Call the function under test
    let result = create_split_rom_memory(&mut buf_reader, offset, total_size, bank_size, CPU_ADDRESS_SPACE);

    assert!(result.is_ok());
    let memory_banks = result.unwrap();
    let num_banks = memory_banks.len();

    assert_eq!(num_banks, expected_num_banks);

    // Verify each bank has correct size and address range
    for bank in &memory_banks {
        assert_eq!(bank.size(), bank_size);
        assert_eq!(bank.get_virtual_address_range(), CPU_ADDRESS_SPACE);
    }

    // Verify the data was read from the correct offset (bytes 4-7 of test_data)
    let expected_data_bank1 = &test_data[4..6]; // 0x34, 0x56
    let expected_data_bank2 = &test_data[6..8]; // 0x78, 0x9A

    for (i, &expected_byte) in expected_data_bank1.iter().enumerate() {
        let actual_byte = memory_banks[0].read_byte(i as u16).expect("failed to read byte from bank 0");
        assert_eq!(actual_byte, expected_byte);
    }

    for (i, &expected_byte) in expected_data_bank2.iter().enumerate() {
        let actual_byte = memory_banks[1].read_byte(i as u16).expect("failed to read byte from bank 1");
        assert_eq!(actual_byte, expected_byte);
    }
}

#[test]
fn create_chr_rom_memory_creates_correct_memory_banks_from_file() {
    init();

    let test_data = vec![0xAB, 0xCD, 0xEF, 0x12, 0x34, 0x56, 0x78, 0x9A];
    let chr_rom_offset = 0u64;
    let chr_rom_total_size = 8;
    let chr_rom_bank_size = 4;
    let expected_num_banks = 2;

    // Create a temporary file with test data
    let mut temp_file = NamedTempFile::new().expect("failed to create temp file");
    temp_file.write_all(&test_data).expect("failed to write test data");
    temp_file.seek(SeekFrom::Start(0)).expect("failed to seek to start");

    let file = temp_file.reopen().expect("failed to reopen temp file");
    let mut buf_reader = BufReader::new(file);

    // Call the function under test
    let result = create_chr_rom_memory(&mut buf_reader, chr_rom_offset, chr_rom_total_size, chr_rom_bank_size, CPU_ADDRESS_SPACE);

    assert!(result.is_ok());
    let memory_banks = result.unwrap();
    let num_banks = memory_banks.len();

    assert_eq!(num_banks, expected_num_banks);

    // Verify each bank has correct size and address range
    for bank in &memory_banks {
        assert_eq!(bank.size(), chr_rom_bank_size);
        assert_eq!(bank.get_virtual_address_range(), CPU_ADDRESS_SPACE);
    }

    // Verify the data was loaded correctly into banks
    let expected_data_bank1 = &test_data[0..4]; // 0xAB, 0xCD, 0xEF, 0x12
    let expected_data_bank2 = &test_data[4..8]; // 0x34, 0x56, 0x78, 0x9A

    for (i, &expected_byte) in expected_data_bank1.iter().enumerate() {
        let actual_byte = memory_banks[0].read_byte(i as u16).expect("failed to read byte from bank 0");
        assert_eq!(actual_byte, expected_byte);
    }

    for (i, &expected_byte) in expected_data_bank2.iter().enumerate() {
        let actual_byte = memory_banks[1].read_byte(i as u16).expect("failed to read byte from bank 1");
        assert_eq!(actual_byte, expected_byte);
    }
}

#[test]
fn create_chr_ram_memory_creates_correct_memory_banks() {
    init();

    let chr_ram_total_size = 8192; // 8KB
    let chr_ram_bank_size = 2048;  // 2KB
    let expected_num_banks = 4; // 4 banks

    let result = create_chr_ram_memory(chr_ram_total_size, chr_ram_bank_size, CPU_ADDRESS_SPACE);

    assert!(result.is_ok());
    let memory_banks = result.unwrap();
    let num_banks = memory_banks.len();

    assert_eq!(num_banks, expected_num_banks);

    // Verify each bank has correct size and address range
    for bank in memory_banks {
        assert_eq!(bank.size(), chr_ram_bank_size);
        assert_eq!(bank.get_virtual_address_range(), CPU_ADDRESS_SPACE);
    }
}

#[test]
fn get_first_bank_or_fail_returns_first_bank() {
    init();

    let memory_banks = [MemoryBank::new(16, CPU_ADDRESS_SPACE)];
    let total_size = 8192;
    let bank_size = 4096;
    let is_rom = true;

    let result = get_first_bank_or_fail(memory_banks.into(), total_size, bank_size, is_rom);
    assert!(result.is_ok());
    assert_eq!(result.unwrap().size(), 16);
}

#[test]
fn get_first_bank_or_fail_returns_error_when_more_than_1_bank_is_present() {
    init();

    let memory_banks = [MemoryBank::new(16, CPU_ADDRESS_SPACE), MemoryBank::new(16, CPU_ADDRESS_SPACE)];
    let total_size = 8192;
    let bank_size = 4096;
    let is_rom = true;

    let result = get_first_bank_or_fail(memory_banks.into(), total_size, bank_size, is_rom);
    assert!(result.is_err());
}