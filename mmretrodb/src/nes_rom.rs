use rmpv::Value;
use crate::rdb::{RdbError};

#[derive(Debug)]
pub struct NesRom {
    name: String,
    region: String,
    rom_name: String,
    size: u64,
    crc: u32,
    md5: [u8; 16],
    sha1: [u8; 20],
    release: NesRomRelease
}

#[derive(Debug)]
pub struct NesRomRelease {
    year: u16,
    month: u8,
}

impl NesRomRelease {
    fn new(year: u64, month: u64) -> Self {
        NesRomRelease {
            year: year as u16,
            month: month.clamp(1, 12) as u8
        }
    }

    pub fn date(&self) -> String {
        format!("{:04}-{:02}", self.year, self.month)
    }
}


impl TryFrom<Value> for NesRom {
    type Error = RdbError;

    fn try_from(value: Value) -> Result<Self, Self::Error> {
        if value.is_map() == false {
            return Err(RdbError::InvalidEntry)
        }

        NesRom::from_messagepack(&value)
    }
}

impl NesRom {

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn region(&self) -> &str {
        self.region.as_str()
    }

    pub fn rom_name(&self) -> &str {
        self.rom_name.as_str()
    }

    pub fn size(&self) -> u64 {
        self.size
    }

    pub fn crc(&self) -> u32 {
        self.crc
    }

    pub fn md5(&self) -> &[u8; 16] {
        &self.md5
    }

    pub fn sha1(&self) -> &[u8; 20] {
        &self.sha1
    }

    pub fn release(&self) -> &NesRomRelease {
        &self.release
    }

    fn get_field<'a>(map: &'a [(Value, Value)], keys: &[&str]) -> Option<&'a Value> {
        for key in keys {
            if let Some((_, v)) = map.iter().find(|(k, _)| k.as_str() == Some(*key)) {
                return Some(v);
            }
        }
        None
    }

    fn get_string(map: &[(Value, Value)], keys: &[&str]) -> Result<String, RdbError> {
        let value = Self::get_field(map, keys).ok_or(RdbError::InvalidEntry)?;
        value.as_str().map(|s| s.to_owned()).ok_or(RdbError::Layout)
    }

    fn get_u64(map: &[(Value, Value)], keys: &[&str]) -> Result<u64, RdbError> {
        let value = Self::get_field(map, keys).ok_or(RdbError::InvalidEntry)?;
        value.as_u64().ok_or(RdbError::Layout)
    }

    fn get_bin_exact<const N: usize>(map: &[(Value, Value)], keys: &[&str]) -> Result<[u8; N], RdbError> {
        let value = Self::get_field(map, keys).ok_or(RdbError::InvalidEntry)?;

        match value {
            Value::Binary(bytes) => bytes.as_slice().try_into().map_err(|_| RdbError::Layout),
            _ => Err(RdbError::Layout),
        }
    }

    fn get_crc_be(map: &[(Value, Value)], keys: &[&str]) -> Result<u32, RdbError> {
        let raw: [u8; 4] = Self::get_bin_exact::<4>(map, keys)?;
        Ok(u32::from_be_bytes(raw))
    }


    fn from_messagepack(entry: &Value) -> Result<NesRom, RdbError> {
        let map = entry.as_map().ok_or(RdbError::InvalidEntry)?;

        let name     = Self::get_string(map, &["name"])?;
        let region   = Self::get_string(map, &["region"]).unwrap_or("(unknown)".to_string());
        let rom_name = Self::get_string(map, &["rom.name", "rom_name"]).unwrap_or("(unknown)".to_string());

        let size     = Self::get_u64(map, &["rom.size", "size"]).unwrap_or(0);

        let crc      = Self::get_crc_be(map, &["rom.crc", "crc"]).unwrap_or(0);
        let md5:  [u8; 16] = Self::get_bin_exact::<16>(map, &["rom.md5", "md5"]).unwrap_or([0; 16]);
        let sha1: [u8; 20] = Self::get_bin_exact::<20>(map, &["rom.sha1", "sha1"]).unwrap_or([0; 20]);

        let month = Self::get_u64(map, &["releasemonth"]).unwrap_or(1);
        let year  = Self::get_u64(map, &["releaseyear"]).unwrap_or(1970);
        let release = NesRomRelease::new(year, month);

        Ok(NesRom { name, region, rom_name, size, crc, md5, sha1, release })
    }
}