

#[allow(non_camel_case_types)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub enum NesMapper {
    NROM,          // 0
    MMC1,          // 1  (SxROM)
    UxROM,         // 2  (UNROM/UOROM)
    CNROM,         // 3
    MMC3,          // 4  (TxROM)
    MMC5,          // 5
    FFE_F4xx,      // 6  (rare)
    AxROM,         // 7  (AOROM/AMROM/ANROM/ASROM)
    MMC2,          // 9
    MMC4,          // 10
    ColorDreams,   // 11
    CPROM,         // 13
    BandaiFCG,     // 16 (Bandai CFG)
    BNROM,         // 34 (NINA-001/BNROM)
    GxROM,         // 66 (GNROM/MHROM)
    Sunsoft4,      // 68
    Sunsoft5B,     // 69 (a.k.a. FME-7)
    Bandai,        // 70 (Bandai LZ93)
    Camerica,      // 71 (Camerica/Codemasters)
    VRC3,          // 73 (Konami)
    VRC1,          // 75 (Konami)
    Irem_H3001,    // 78 (Irem 74HC161/32)
    NINA_003_006,  // 79
    VRC7,          // 85
    Jaleco_SS8806, // 18
    Namco163,      // 19
    VRC2A,         // 22
    VRC4,          // 21/23 (grouped under one name here)
    VRC6,          // 24
    Taito_TC0190,  // 33 (Taito TC0190/TC0350)
    Taito_X1_005,  // 80 (optional if you use it)
    TxSROM,        // 118 (MMC3 variant with CHR-RAM banking)
    TQROM,         // 119 (MMC3 + CHR-RAM/ROM mix)
    Unknown(u16),
}

impl NesMapper {
    pub const fn id(self) -> u16 {
        match self {
            NesMapper::NROM           => 0,
            NesMapper::MMC1           => 1,
            NesMapper::UxROM          => 2,
            NesMapper::CNROM          => 3,
            NesMapper::MMC3           => 4,
            NesMapper::MMC5           => 5,
            NesMapper::FFE_F4xx       => 6,
            NesMapper::AxROM          => 7,
            NesMapper::MMC2           => 9,
            NesMapper::MMC4           => 10,
            NesMapper::ColorDreams    => 11,
            NesMapper::CPROM          => 13,
            NesMapper::BandaiFCG      => 16,
            NesMapper::BNROM          => 34,
            NesMapper::GxROM          => 66,
            NesMapper::Sunsoft4       => 68,
            NesMapper::Sunsoft5B      => 69,
            NesMapper::Bandai         => 70,
            NesMapper::Camerica       => 71,
            NesMapper::VRC3           => 73,
            NesMapper::VRC1           => 75,
            NesMapper::Irem_H3001     => 78,
            NesMapper::NINA_003_006   => 79,
            NesMapper::VRC7           => 85,
            NesMapper::Jaleco_SS8806  => 18,
            NesMapper::Namco163       => 19,
            NesMapper::VRC2A          => 22,
            NesMapper::VRC4           => 23, // one of the VRC4 IDs
            NesMapper::VRC6           => 24,
            NesMapper::Taito_TC0190   => 33,
            NesMapper::Taito_X1_005   => 80,
            NesMapper::TxSROM         => 118,
            NesMapper::TQROM          => 119,
            NesMapper::Unknown(id)    => id,
        }
    }

    pub const fn name(self) -> &'static str {
        match self {
            NesMapper::NROM           => "NROM",
            NesMapper::MMC1           => "MMC1 (SxROM)",
            NesMapper::UxROM          => "UxROM (UNROM/UOROM)",
            NesMapper::CNROM          => "CNROM",
            NesMapper::MMC3           => "MMC3 (TxROM)",
            NesMapper::MMC5           => "MMC5",
            NesMapper::FFE_F4xx       => "FFE F4xx",
            NesMapper::AxROM          => "AxROM (AOROM/AMROM/ANROM/ASROM)",
            NesMapper::MMC2           => "MMC2",
            NesMapper::MMC4           => "MMC4",
            NesMapper::ColorDreams    => "Color Dreams",
            NesMapper::CPROM          => "CPROM",
            NesMapper::BandaiFCG      => "Bandai FCG",
            NesMapper::BNROM          => "BNROM / NINA-001",
            NesMapper::GxROM          => "GxROM (GNROM/MHROM)",
            NesMapper::Sunsoft4       => "Sunsoft 4",
            NesMapper::Sunsoft5B      => "Sunsoft 5B (FME-7)",
            NesMapper::Bandai         => "Bandai LZ93",
            NesMapper::Camerica       => "Camerica/Codemasters",
            NesMapper::VRC3           => "Konami VRC3",
            NesMapper::VRC1           => "Konami VRC1",
            NesMapper::Irem_H3001     => "Irem (H-3001)",
            NesMapper::NINA_003_006   => "NINA-003/006",
            NesMapper::VRC7           => "Konami VRC7",
            NesMapper::Jaleco_SS8806  => "Jaleco SS8806",
            NesMapper::Namco163       => "Namco 163",
            NesMapper::VRC2A          => "Konami VRC2A",
            NesMapper::VRC4           => "Konami VRC4",
            NesMapper::VRC6           => "Konami VRC6",
            NesMapper::Taito_TC0190   => "Taito TC0190/TC0350",
            NesMapper::Taito_X1_005   => "Taito X1-005",
            NesMapper::TxSROM         => "TxSROM (MMC3 variant)",
            NesMapper::TQROM          => "TQROM (MMC3 variant)",
            NesMapper::Unknown(_)     => "Unknown",
        }
    }

    pub const fn is_supported(self) -> bool {
        match self {
            NesMapper::NROM => true,
            _ => false,
        }
    }

    pub const fn from_id(id: u16) -> Self {
        match id {
            0 => NesMapper::NROM,
            1 => NesMapper::MMC1,
            2 => NesMapper::UxROM,
            3 => NesMapper::CNROM,
            4 => NesMapper::MMC3,
            5 => NesMapper::MMC5,
            6 => NesMapper::FFE_F4xx,
            7 => NesMapper::AxROM,
            9 => NesMapper::MMC2,
            10 => NesMapper::MMC4,
            11 => NesMapper::ColorDreams,
            13 => NesMapper::CPROM,
            16 => NesMapper::BandaiFCG,
            18 => NesMapper::Jaleco_SS8806,
            19 => NesMapper::Namco163,
            22 => NesMapper::VRC2A,
            23 => NesMapper::VRC4,
            24 => NesMapper::VRC6,
            33 => NesMapper::Taito_TC0190,
            34 => NesMapper::BNROM,
            66 => NesMapper::GxROM,
            68 => NesMapper::Sunsoft4,
            69 => NesMapper::Sunsoft5B,
            70 => NesMapper::Bandai,
            71 => NesMapper::Camerica,
            73 => NesMapper::VRC3,
            75 => NesMapper::VRC1,
            78 => NesMapper::Irem_H3001,
            79 => NesMapper::NINA_003_006,
            80 => NesMapper::Taito_X1_005,
            85 => NesMapper::VRC7,
            118 => NesMapper::TxSROM,
            119 => NesMapper::TQROM,
            _   => NesMapper::Unknown(id),
        }
    }
}