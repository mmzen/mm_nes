use std::fs;
use std::io::Write;
use std::path::Path;
use tempfile::NamedTempFile;
use byteorder::{BigEndian, WriteBytesExt};
use rmpv::encode;
use crate::rdb::{Rdb, RdbError};
use crate::tests::init;

const RDB_NES_FILE_PATH: &str = "src/assets/nes.rdb";

#[test]
fn test_open_nes_rdb_file() {
    init();

    let metadata = fs::metadata(RDB_NES_FILE_PATH).unwrap();
    let rdb = Rdb::open(Path::new(RDB_NES_FILE_PATH)).unwrap();

    assert_eq!(rdb.items_count(), 29390);
    assert_eq!(rdb.file_len(), metadata.len());
}

#[test]
fn test_scan_by_crc_with_existing_crc_and_full_record() {
    init();

    let mut rdb = Rdb::open(Path::new(RDB_NES_FILE_PATH)).unwrap();
    let rom = rdb.scan_by_crc(872268823).unwrap().unwrap();

    assert_eq!(rom.name().unwrap(), "Akira (Japan)[h][t][u][mapper 4]");
    assert_eq!(rom.region().unwrap(), "Japan");
    assert_eq!(rom.rom_name().unwrap(), "Akira (1988-12-24)(Taito)(JP)[h][t][u][mapper 4].nes");
    assert_eq!(rom.size().unwrap(), 393232);
    assert_eq!(rom.crc().unwrap(), 872268823);
    assert_eq!(rom.md5().unwrap(), &[9, 39, 224, 85, 166, 204, 70, 214, 82, 71, 217, 112, 25, 231, 176, 93]);
    assert_eq!(rom.sha1().unwrap(), &[164, 30, 8, 75, 97, 19, 36, 36, 48, 180, 118, 211, 176, 218, 28, 141, 184, 45, 168, 61]);
    assert_eq!(rom.release().unwrap().date(), "1988-12");
}

#[test]
fn test_scan_by_crc_with_existing_crc_and_partial_record() {
    init();

    let mut rdb = Rdb::open(Path::new(RDB_NES_FILE_PATH)).unwrap();
    let rom = rdb.scan_by_crc(381528540).unwrap().unwrap();

    assert_eq!(rom.name().unwrap(), "100 in 1 Contra Function 16 (-) (Asia)[h][p][b5][iNES title]");
    assert_eq!(rom.region().unwrap(), "Asia");
    assert_eq!(rom.rom_name().unwrap(), "100 in 1 Contra Function 16 (19xx)(-)(AS)[h][p][b5][iNES title].nes");
    assert_eq!(rom.size().unwrap(), 1048720);
    assert_eq!(rom.crc().unwrap(), 381528540);
    assert_eq!(rom.md5().unwrap(), &[152, 207, 189, 176, 189, 215, 76, 235, 152, 52, 84, 163, 117, 103, 200, 164]);
    assert_eq!(rom.sha1().unwrap(), &[12, 181, 242, 242, 235, 223, 119, 20, 179, 25, 169, 119, 224, 81, 183, 19, 253, 95, 27, 183]);
    assert_eq!(rom.release(), None);
}

#[test]
fn test_scan_by_crc_not_found() {
    init();

    let mut rdb = Rdb::open(Path::new(RDB_NES_FILE_PATH)).unwrap();
    let rom = rdb.scan_by_crc(0).unwrap();

    assert!(rom.is_none());
}

#[test]
fn test_open_rdb_file_with_bad_magic() {
    init();

    let mut temp_file = NamedTempFile::new().unwrap();

    temp_file.write_all(b"BADMAGIC").unwrap();
    temp_file.write_u64::<BigEndian>(0).unwrap();
    temp_file.flush().unwrap();

    let result = Rdb::open(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(RdbError::BadMagic) => {},
        _ => panic!("Expected BadMagic error"),
    }
}

#[test]
fn test_open_non_existent_file_returns_io_error() {
    init();

    let result = Rdb::open(Path::new("non_existent_file.rdb"));

    assert!(result.is_err());
    match result.unwrap_err() {
        RdbError::Io(_) => {},
        _ => panic!("Expected Io error"),
    }
}

#[test]
fn test_open_rdb_bad_metadata_not_map() {
    init();

    let mut temp_file = NamedTempFile::new().unwrap();

    temp_file.write_all(b"RARCHDB\0").unwrap();
    temp_file.write_u64::<BigEndian>(16).unwrap();

    let invalid_metadata = rmpv::Value::String("not a map".into());
    encode::write_value(&mut temp_file, &invalid_metadata).unwrap();

    temp_file.flush().unwrap();

    let result = Rdb::open(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(RdbError::Layout) => {},
        _ => panic!("expected Layout error"),
    }
}

#[test]
fn test_open_rdb_bad_metadata_missing_count() {
    init();

    let mut temp_file = NamedTempFile::new().unwrap();

    temp_file.write_all(b"RARCHDB\0").unwrap();
    temp_file.write_u64::<BigEndian>(16).unwrap();

    let mut metadata_map = Vec::new();
    metadata_map.push((rmpv::Value::String("other_field".into()), rmpv::Value::Integer(42.into())));
    let metadata = rmpv::Value::Map(metadata_map);
    encode::write_value(&mut temp_file, &metadata).unwrap();

    temp_file.flush().unwrap();

    let result = Rdb::open(temp_file.path());

    assert!(result.is_err());
    match result {
        Err(RdbError::Layout) => {},
        _ => panic!("expected Layout error"),
    }
}

#[test]
fn test_crc32_file_known_content() {
    init();

    let expected_crc = 0xe5b26e1f;
    let calculated_crc = Rdb::crc32(RDB_NES_FILE_PATH).unwrap();

    assert_eq!(calculated_crc, expected_crc);
}
