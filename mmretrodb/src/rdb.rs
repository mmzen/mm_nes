use std::fs;
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::path::Path;
use byteorder::{BigEndian, ReadBytesExt};
use crc32fast::Hasher;
use log::debug;
use rmpv::{decode, Value};
use crate::nes_rom::NesRom;

#[derive(Debug)]
pub enum RdbError {
    Io(String),
    MessagePack(),
    BadMagic,
    Layout,
    InvalidEntry
}

impl From<std::io::Error> for RdbError {
    fn from(error: std::io::Error) -> Self {
        RdbError::Io(error.to_string())
    }
}

impl From<decode::Error> for RdbError {
    fn from(_: decode::Error) -> Self {
        RdbError::MessagePack()
    }
}

#[derive(Debug)]
pub struct Rdb {
    file: BufReader<File>,
    items_offset: u64,
    items_count: u64,
    file_len: u64,
}

impl Rdb {

    #[cfg(test)]
    pub fn items_count(&self) -> u64 {
        self.items_count
    }

    #[cfg(test)]
    pub fn file_len(&self) -> u64 {
        self.file_len
    }

    fn open_rdb_file(path: impl AsRef<Path>) -> Result<(BufReader<File>, u64), RdbError> {
        let file = File::open(path)?;
        let file_len = file.metadata()?.len();
        debug!("file length: {}", file_len);

        Ok((BufReader::new(file), file_len))
    }

    fn verify_header(file: &mut BufReader<File>) -> Result<(), RdbError> {
        let mut magic = [0u8; 8];

        file.read_exact(&mut magic)?;
        debug!("magic: {:?}", &magic);

        if &magic[..7] != b"RARCHDB" {
            Err(RdbError::BadMagic)
        } else {
            Ok(())
        }
    }

    fn metadata_offset(file: &mut BufReader<File>) -> Result<u64, RdbError> {
        let offset = file.read_u64::<BigEndian>()?;
        debug!("metadata offset: {}", offset);

        Ok(offset)
    }

    fn items_offset(file: &mut BufReader<File>) -> Result<u64, RdbError> {
        let offset = file.seek(SeekFrom::Current(0))?;
        debug!("items offset: {}", offset);

        Ok(offset)
    }

    pub(crate) fn value_by_key<'a>(entry: &'a Value, key: &str) -> Result<Option<&'a Value>, RdbError> {
        let map = entry.as_map().ok_or(RdbError::Layout)?;

        let hit = map.iter().find_map(|(k, v)| {
            if k.as_str() == Some(key) {
                Some(v)
            } else {
                None
            }
        });

        Ok(hit)
    }

    fn count(file: &mut BufReader<File>, offset: u64) -> Result<u64, RdbError> {
        file.seek(SeekFrom::Start(offset))?;

        let value = decode::read_value(file)?;
        debug!("value: {:?}", value);

        let value = Rdb::value_by_key(&value, "count")?.ok_or(RdbError::Layout)?;
        let count = value.as_u64().ok_or(RdbError::Layout)?;

        debug!("items count: {}", count);
        Ok(count)
    }

    pub fn open(path: impl AsRef<Path>) -> Result<Rdb, RdbError> {
        let (mut file, file_len) = Rdb::open_rdb_file(path)?;

        Rdb::verify_header(&mut file)?;

        let metadata_offset = Rdb::metadata_offset(&mut file)?;
        let items_offset = Rdb::items_offset(&mut file)?;
        let count = Rdb::count(&mut file, metadata_offset)?;

        Ok(Rdb {
            file,
            items_offset,
            items_count: count,
            file_len,
        })
    }

    pub(crate) fn array_to_u32(bytes: &[u8]) -> Result<u32, RdbError> {
        let array: [u8; 4] = bytes.try_into().map_err(|_| RdbError::Layout)?;
        Ok(u32::from_be_bytes(array))
    }

    fn match_by_crc(entry: Value, crc: u32) -> Result<Option<Value>, RdbError> {
        let value = Rdb::value_by_key(&entry, "crc")?.ok_or(RdbError::Layout)?;

        match value {
            Value::Binary(binary) => {
                let entry_crc = Rdb::array_to_u32(&binary)?;
                if entry_crc == crc {
                    return Ok(Some(entry));
                }
            },

            _ => {}
        }

        Ok(None)
    }

    pub fn scan_by_crc(&mut self, crc: u32) -> Result<Option<NesRom>, RdbError> {
        let eof = self.file_len;

        self.file.seek(SeekFrom::Start(self.items_offset))?;

        while self.items_offset < eof {
            let entry = decode::read_value(&mut self.file)?;

            if entry.is_nil() {
                break;
            }

            match Rdb::match_by_crc(entry, crc) {
                Ok(Some(entry)) => {
                    let rom = NesRom::try_from(entry)?;
                    return Ok(Some(rom));
                },
                Ok(None) => {},
                Err(e) => return Err(e),
            }
        }

        Ok(None)
    }

    pub fn crc32(path: &str) -> Result<u32, RdbError> {
        let file = File::open(path)?;
        let mut reader = BufReader::new(file);
        let mut hasher = Hasher::new();
        let mut buffer = [0u8; 64 * 1024];

        loop {
            let n = reader.read(&mut buffer)?;

            if n == 0 {
                break;
            }

            hasher.update(&buffer[..n]);
        }

        Ok(hasher.finalize())
    }
}