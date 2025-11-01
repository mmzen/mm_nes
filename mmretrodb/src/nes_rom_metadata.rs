use rmpv::Value;
use crate::rdb::{RdbError};

#[derive(Debug)]
pub struct NesRomMetadata {
    name: Option<String>,
    region: Option<String>,
    rom_name: Option<String>,
    size: Option<u64>,
    crc: Option<u32>,
    md5: Option<[u8; 16]>,
    sha1: Option<[u8; 20]>,
    release: Option<NesRomRelease>
}

#[derive(Debug, PartialEq)]
pub struct NesRomRelease {
    year: u16,
    month: Option<u8>,
}

impl NesRomRelease {
    fn new(year: u64, month: Option<u64>) -> Self {
        NesRomRelease {
            year: year as u16,
            month: month.map(|m| m.clamp(1, 12) as u8) }
    }

    pub fn date(&self) -> String {
        if let Some(month) = self.month {
            format!("{:04}-{:02}", self.year, month)
        } else {
            format!("{:04}", self.year)
        }
    }
}


impl TryFrom<Value> for NesRomMetadata {
    type Error = RdbError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value.is_map() == false {
            return Err(RdbError::InvalidEntry)
        }

        NesRomMetadata::from_messagepack(&value)
    }
}

impl NesRomMetadata {

    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    pub fn region(&self) -> Option<&str> {
        self.region.as_deref()
    }

    pub fn rom_name(&self) -> Option<&str> {
        self.rom_name.as_deref()
    }

    pub fn size(&self) -> Option<u64> {
        self.size
    }

    pub fn crc(&self) -> Option<u32> {
        self.crc
    }

    pub fn md5(&self) -> Option<&[u8; 16]> {
        self.md5.as_ref()
    }

    pub fn sha1(&self) -> Option<&[u8; 20]> {
        self.sha1.as_ref()
    }

    pub fn release(&self) -> Option<&NesRomRelease> {
        self.release.as_ref()
    }

    fn get_field<'a>(map: &'a [(Value, Value)], keys: &[&str]) -> Option<&'a Value> {
        for key in keys {
            if let Some((_, v)) = map.iter().find(|(k, _)| k.as_str() == Some(*key)) {
                return Some(v);
            }
        }
        None
    }

    fn get_string(map: &[(Value, Value)], keys: &[&str]) -> Result<Option<String>, RdbError> {
        NesRomMetadata::get_field(map, keys)
            .map(|value| value.as_str().ok_or(RdbError::Layout).map(str::to_owned))
            .transpose()
    }

    fn get_u64(map: &[(Value, Value)], keys: &[&str]) -> Result<Option<u64>, RdbError> {
        NesRomMetadata::get_field(map, keys)
            .map(|value| value.as_u64().ok_or(RdbError::Layout))
            .transpose()
    }

    fn get_bin_exact<const N: usize>(map: &[(Value, Value)], keys: &[&str]) -> Result<Option<[u8; N]>, RdbError> {
        NesRomMetadata::get_field(map, keys)
            .map(|value|
                match value {
                Value::Binary(bytes) => bytes.as_slice().try_into().map_err(|_| RdbError::Layout),
                _ => Err(RdbError::Layout),
            })
            .transpose()
    }

    fn get_crc_be(map: &[(Value, Value)], keys: &[&str]) -> Result<Option<u32>, RdbError> {
        NesRomMetadata::get_bin_exact::<4>(map, keys)?
            .map(|raw| Ok(u32::from_be_bytes(raw)))
            .transpose()
    }


    fn from_messagepack(entry: &Value) -> Result<NesRomMetadata, RdbError> {
        let map = entry.as_map().ok_or(RdbError::InvalidEntry)?;

        let name = NesRomMetadata::get_string(map, &["name"])?;
        let region = NesRomMetadata::get_string(map, &["region"])?;
        let rom_name = NesRomMetadata::get_string(map, &["rom.name", "rom_name"])?;

        let size = NesRomMetadata::get_u64(map, &["rom.size", "size"])?;

        let crc = NesRomMetadata::get_crc_be(map, &["rom.crc", "crc"])?;
        let md5:  Option<[u8; 16]> = NesRomMetadata::get_bin_exact::<16>(map, &["rom.md5", "md5"])?;
        let sha1: Option<[u8; 20]> = NesRomMetadata::get_bin_exact::<20>(map, &["rom.sha1", "sha1"])?;

        let month = NesRomMetadata::get_u64(map, &["releasemonth"])?;
        let year  = NesRomMetadata::get_u64(map, &["releaseyear"])?;

        let release = if let Some(year) = year {
            Some(NesRomRelease::new(year, month))
        } else {
            None
        };

        Ok(NesRomMetadata { name, region, rom_name, size, crc, md5, sha1, release })
    }
}