use once_cell::sync::Lazy;
use std::collections::HashMap;

pub struct InstructionDetails {
    pub(crate) addressing: &'static str,
    pub(crate) assembler: &'static str,
    pub(crate) opc: &'static str,
    pub(crate) bytes: &'static str,
    pub(crate) cycles: &'static str,
}

pub struct ToolTip6502 {
    pub(crate) title: &'static str,
    pub(crate) summary: Option<&'static str>,
    pub(crate) flags_note: Option<&'static str>,
    pub(crate) rows: Vec<InstructionDetails>,
    pub(crate) exception: Option<&'static str>,
}

impl ToolTip6502 {
    pub(crate) fn tooltip(mnemonic: &str) -> Option<&ToolTip6502> {
        TOOLTIP_6502.get(mnemonic)
    }
}

pub static TOOLTIP_6502: Lazy<HashMap<&'static str, ToolTip6502>> = Lazy::new(|| {
    let mut m = HashMap::new();

    // --- ADC ---
    m.insert("ADC", ToolTip6502 {
        title: "ADC",
        summary: Some("Add with Carry (A ← A + M + C)"),
        flags_note: Some("Affects N,Z,C,V; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "ADC #oper",     opc: "69", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "ADC oper",      opc: "65", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "ADC oper,X",    opc: "75", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "ADC oper",      opc: "6D", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "ADC oper,X",    opc: "7D", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "ADC oper,Y",    opc: "79", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "ADC (oper,X)",  opc: "61", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "ADC (oper),Y",  opc: "71", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- AND ---
    m.insert("AND", ToolTip6502 {
        title: "AND",
        summary: Some("AND with Accumulator (A ← A & M)"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "AND #oper",     opc: "29", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "AND oper",      opc: "25", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "AND oper,X",    opc: "35", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "AND oper",      opc: "2D", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "AND oper,X",    opc: "3D", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "AND oper,Y",    opc: "39", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "AND (oper,X)",  opc: "21", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "AND (oper),Y",  opc: "31", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- ASL ---
    m.insert("ASL", ToolTip6502 {
        title: "ASL",
        summary: Some("Arithmetic Shift Left"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Accumulator",   assembler: "ASL A",         opc: "0A", bytes: "1", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "ASL oper",      opc: "06", bytes: "2", cycles: "5"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "ASL oper,X",    opc: "16", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "Absolute",      assembler: "ASL oper",      opc: "0E", bytes: "3", cycles: "6"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "ASL oper,X",    opc: "1E", bytes: "3", cycles: "7"   },
        ],
        exception: None,
    });

    // --- BIT ---
    m.insert("BIT", ToolTip6502 {
        title: "BIT",
        summary: Some("Test Bits (sets Z from A&M; N from M7; V from M6)"),
        flags_note: Some("Affects N,V,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "BIT oper",      opc: "24", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "Absolute",      assembler: "BIT oper",      opc: "2C", bytes: "3", cycles: "4"   },
        ],
        exception: None,
    });

    // --- BRK ---
    m.insert("BRK", ToolTip6502 {
        title: "BRK",
        summary: Some("Force Interrupt; push PC+2 and P; I=1"),
        flags_note: Some("Flags pushed; on pull, B cleared; U=1."),
        rows: vec![
            InstructionDetails { addressing: "implied",       assembler: "BRK",          opc: "00", bytes: "1", cycles: "7"   },
        ],
        exception: None,
    });

    // --- Branches ---
    m.insert("BCC", ToolTip6502 {
        title: "BCC",
        summary: Some("Branch on Carry Clear"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BCC oper", opc: "90", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BCS", ToolTip6502 {
        title: "BCS",
        summary: Some("Branch on Carry Set"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BCS oper", opc: "B0", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BEQ", ToolTip6502 {
        title: "BEQ",
        summary: Some("Branch on Zero Set"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BEQ oper", opc: "F0", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BMI", ToolTip6502 {
        title: "BMI",
        summary: Some("Branch on Negative Set"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BMI oper", opc: "30", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BNE", ToolTip6502 {
        title: "BNE",
        summary: Some("Branch on Zero Clear"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BNE oper", opc: "D0", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BPL", ToolTip6502 {
        title: "BPL",
        summary: Some("Branch on Negative Clear"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BPL oper", opc: "10", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BVC", ToolTip6502 {
        title: "BVC",
        summary: Some("Branch on Overflow Clear"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BVC oper", opc: "50", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    m.insert("BVS", ToolTip6502 {
        title: "BVS",
        summary: Some("Branch on Overflow Set"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "relative", assembler: "BVS oper", opc: "70", bytes: "2", cycles: "2**" }
        ],
        exception: Some("** add 1 to cycles if branch occurs on same page, add 2 to cycles if branch occurs to different page")
    });

    // --- Flag ops ---
    m.insert("CLC", ToolTip6502 {
        title: "CLC",
        summary: Some("Clear Carry"),
        flags_note: Some("C=0; others unchanged."),
        rows: vec![ InstructionDetails {
            addressing: "implied", assembler: "CLC", opc: "18", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("CLD", ToolTip6502 {
        title: "CLD",
        summary: Some("Clear Decimal"),
        flags_note: Some("D=0; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "CLD", opc: "D8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("CLI", ToolTip6502 {
        title: "CLI", summary: Some("Clear Interrupt Disable"),
        flags_note: Some("I=0; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "CLI", opc: "58", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("CLV", ToolTip6502 {
        title: "CLV", summary: Some("Clear Overflow"),
        flags_note: Some("V=0; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "CLV", opc: "B8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("SEC", ToolTip6502 {
        title: "SEC", summary: Some("Set Carry"),
        flags_note: Some("C=1; others unchanged."), rows: vec![
            InstructionDetails { addressing: "implied", assembler: "SEC", opc: "38", bytes: "1", cycles: "2" }
        ] ,
        exception: None,
    });

    m.insert("SED", ToolTip6502 {
        title: "SED", summary: Some("Set Decimal"),
        flags_note: Some("D=1; others unchanged."), rows: vec![
            InstructionDetails { addressing: "implied", assembler: "SED", opc: "F8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("SEI", ToolTip6502 {
        title: "SEI", summary: Some("Set Interrupt Disable"),
        flags_note: Some("I=1; others unchanged."), rows: vec![
            InstructionDetails { addressing: "implied", assembler: "SEI", opc: "78", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    // --- CMP / CPX / CPY ---
    m.insert("CMP", ToolTip6502 {
        title: "CMP",
        summary: Some("Compare A with M (A-M)"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "CMP #oper",     opc: "C9", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "CMP oper",      opc: "C5", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "CMP oper,X",    opc: "D5", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "CMP oper",      opc: "CD", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "CMP oper,X",    opc: "DD", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "CMP oper,Y",    opc: "D9", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "CMP (oper,X)",  opc: "C1", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "CMP (oper),Y",  opc: "D1", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    m.insert("CPX", ToolTip6502 {
        title: "CPX",
        summary: Some("Compare X with M (X-M)"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "CPX #oper",     opc: "E0", bytes: "2", cycles: "2" },
            InstructionDetails { addressing: "zero Page",     assembler: "CPX oper",      opc: "E4", bytes: "2", cycles: "3" },
            InstructionDetails { addressing: "Absolute",      assembler: "CPX oper",      opc: "EC", bytes: "3", cycles: "4" },
        ],
        exception: None,
    });

    m.insert("CPY", ToolTip6502 {
        title: "CPY",
        summary: Some("Compare Y with M (Y-M)"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "CPY #oper",     opc: "C0", bytes: "2", cycles: "2" },
            InstructionDetails { addressing: "zero Page",     assembler: "CPY oper",      opc: "C4", bytes: "2", cycles: "3" },
            InstructionDetails { addressing: "Absolute",      assembler: "CPY oper",      opc: "CC", bytes: "3", cycles: "4" },
        ],
        exception: None,
    });

    // --- DEC / DEX / DEY ---
    m.insert("DEC", ToolTip6502 {
        title: "DEC",
        summary: Some("Decrement Memory"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "DEC oper",      opc: "C6", bytes: "2", cycles: "5" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "DEC oper,X",    opc: "D6", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "Absolute",      assembler: "DEC oper",      opc: "CE", bytes: "3", cycles: "6" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "DEC oper,X",    opc: "DE", bytes: "3", cycles: "7" },
        ],
        exception: None,
    });

    m.insert("DEX", ToolTip6502 {
        title: "DEX",
        summary: Some("Decrement X"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "DEX", opc: "CA", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("DEY", ToolTip6502 {
        title: "DEY",
        summary: Some("Decrement Y"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "DEY", opc: "88", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    // --- EOR ---
    m.insert("EOR", ToolTip6502 {
        title: "EOR",
        summary: Some("Exclusive OR (A ← A ^ M)"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "EOR #oper",     opc: "49", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "EOR oper",      opc: "45", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "EOR oper,X",    opc: "55", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "EOR oper",      opc: "4D", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "EOR oper,X",    opc: "5D", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "EOR oper,Y",    opc: "59", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "EOR (oper,X)",  opc: "41", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "EOR (oper),Y",  opc: "51", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- INC / INX / INY ---
    m.insert("INC", ToolTip6502 {
        title: "INC",
        summary: Some("Increment Memory"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "INC oper",      opc: "E6", bytes: "2", cycles: "5" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "INC oper,X",    opc: "F6", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "Absolute",      assembler: "INC oper",      opc: "EE", bytes: "3", cycles: "6" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "INC oper,X",    opc: "FE", bytes: "3", cycles: "7" },
        ],
        exception: None,
    });

    m.insert("INX", ToolTip6502 {
        title: "INX",
        summary: Some("Increment X"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "INX", opc: "E8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("INY", ToolTip6502 {
        title: "INY",
        summary: Some("Increment Y"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "INY", opc: "C8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    // --- JMP / JSR ---
    m.insert("JMP", ToolTip6502 {
        title: "JMP",
        summary: Some("Jump"),
        flags_note: Some("Flags unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Absolute",      assembler: "JMP oper",      opc: "4C", bytes: "3", cycles: "3" },
            InstructionDetails { addressing: "(indirect)",    assembler: "JMP (oper)",    opc: "6C", bytes: "3", cycles: "5" },
        ],
        exception: None,
    });

    m.insert("JSR", ToolTip6502 {
        title: "JSR",
        summary: Some("Jump to Subroutine"),
        flags_note: Some("Push return address; flags unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Absolute",      assembler: "JSR oper",      opc: "20", bytes: "3", cycles: "6" },
        ],
        exception: None,
    });

    // --- LDA / LDX / LDY ---
    m.insert("LDA", ToolTip6502 {
        title: "LDA",
        summary: Some("Load Accumulator"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "LDA #oper",     opc: "A9", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "LDA oper",      opc: "A5", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "LDA oper,X",    opc: "B5", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "LDA oper",      opc: "AD", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "LDA oper,X",    opc: "BD", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "LDA oper,Y",    opc: "B9", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "LDA (oper,X)",  opc: "A1", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "LDA (oper),Y",  opc: "B1", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    m.insert("LDX", ToolTip6502 {
        title: "LDX",
        summary: Some("Load X"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "LDX #oper",     opc: "A2", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "LDX oper",      opc: "A6", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,Y",   assembler: "LDX oper,Y",    opc: "B6", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "LDX oper",      opc: "AE", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "LDX oper,Y",    opc: "BE", bytes: "3", cycles: "4*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    m.insert("LDY", ToolTip6502 {
        title: "LDY",
        summary: Some("Load Y"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "LDY #oper",     opc: "A0", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "LDY oper",      opc: "A4", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "LDY oper,X",    opc: "B4", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "LDY oper",      opc: "AC", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "LDY oper,X",    opc: "BC", bytes: "3", cycles: "4*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- LSR ---
    m.insert("LSR", ToolTip6502 {
        title: "LSR",
        summary: Some("Logical Shift Right"),
        flags_note: Some("Affects N(=0),Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Accumulator",   assembler: "LSR A",         opc: "4A", bytes: "1", cycles: "2" },
            InstructionDetails { addressing: "zero Page",     assembler: "LSR oper",      opc: "46", bytes: "2", cycles: "5" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "LSR oper,X",    opc: "56", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "Absolute",      assembler: "LSR oper",      opc: "4E", bytes: "3", cycles: "6" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "LSR oper,X",    opc: "5E", bytes: "3", cycles: "7" },
        ],
        exception: None,
    });

    // --- NOP ---
    m.insert("NOP", ToolTip6502 {
        title: "NOP",
        summary: Some("No Operation"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "NOP", opc: "EA", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    // --- ORA ---
    m.insert("ORA", ToolTip6502 {
        title: "ORA",
        summary: Some("OR with Accumulator (A ← A | M)"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "ORA #oper",     opc: "09", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "ORA oper",      opc: "05", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "ORA oper,X",    opc: "15", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "ORA oper",      opc: "0D", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "ORA oper,X",    opc: "1D", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "ORA oper,Y",    opc: "19", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "ORA (oper,X)",  opc: "01", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "ORA (oper),Y",  opc: "11", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- PHA / PHP / PLA / PLP ---
    m.insert("PHA", ToolTip6502 {
        title: "PHA",
        summary: Some("Push A"),
        flags_note: Some("Flags unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "PHA", opc: "48", bytes: "1", cycles: "3" }
        ],
        exception: None,
    });

    m.insert("PHP", ToolTip6502 {
        title: "PHP",
        summary: Some("Push Processor Status (with B=1,U=1)"),
        flags_note: Some("Stack write; flags unchanged."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "PHP", opc: "08", bytes: "1", cycles: "3" }
        ],
        exception: None,
    });

    m.insert("PLA", ToolTip6502 {
        title: "PLA",
        summary: Some("Pull A"),
        flags_note: Some("Affects N,Z from A."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "PLA", opc: "68", bytes: "1", cycles: "4" }
        ],
        exception: None,
    });

    m.insert("PLP", ToolTip6502 {
        title: "PLP", summary: Some("Pull Processor Status"),
        flags_note: Some("Restores flags (B cleared on storage)."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "PLP", opc: "28", bytes: "1", cycles: "4" }
        ],
        exception: None,
    });

    // --- ROL / ROR ---
    m.insert("ROL", ToolTip6502 {
        title: "ROL",
        summary: Some("Rotate Left through Carry"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Accumulator",   assembler: "ROL A",         opc: "2A", bytes: "1", cycles: "2" },
            InstructionDetails { addressing: "zero Page",     assembler: "ROL oper",      opc: "26", bytes: "2", cycles: "5" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "ROL oper,X",    opc: "36", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "Absolute",      assembler: "ROL oper",      opc: "2E", bytes: "3", cycles: "6" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "ROL oper,X",    opc: "3E", bytes: "3", cycles: "7" },
        ],
        exception: None,
    });

    m.insert("ROR", ToolTip6502 {
        title: "ROR",
        summary: Some("Rotate Right through Carry"),
        flags_note: Some("Affects N,Z,C; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "Accumulator",   assembler: "ROR A",         opc: "6A", bytes: "1", cycles: "2" },
            InstructionDetails { addressing: "zero Page",     assembler: "ROR oper",      opc: "66", bytes: "2", cycles: "5" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "ROR oper,X",    opc: "76", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "Absolute",      assembler: "ROR oper",      opc: "6E", bytes: "3", cycles: "6" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "ROR oper,X",    opc: "7E", bytes: "3", cycles: "7" },
        ],
        exception: None,
    });

    // --- RTI / RTS ---
    m.insert("RTI", ToolTip6502 {
        title: "RTI", summary: Some("Return from Interrupt"),
        flags_note: Some("Pull P then PC."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "RTI", opc: "40", bytes: "1", cycles: "6" }
        ],
        exception: None,
    });

    m.insert("RTS", ToolTip6502 {
        title: "RTS",
        summary: Some("Return from Subroutine"),
        flags_note: Some("Pull PC then increment."),
        rows: vec![
            InstructionDetails { addressing: "implied", assembler: "RTS", opc: "60", bytes: "1", cycles: "6" }
        ],
        exception: None,
    });

    // --- SBC ---
    m.insert("SBC", ToolTip6502 {
        title: "SBC",
        summary: Some("Subtract with Borrow (A ← A - M - (1-C))"),
        flags_note: Some("Affects N,Z,C,V; others unchanged."),
        rows: vec![
            InstructionDetails { addressing: "immediate",     assembler: "SBC #oper",     opc: "E9", bytes: "2", cycles: "2"   },
            InstructionDetails { addressing: "zero Page",     assembler: "SBC oper",      opc: "E5", bytes: "2", cycles: "3"   },
            InstructionDetails { addressing: "zero Page,X",   assembler: "SBC oper,X",    opc: "F5", bytes: "2", cycles: "4"   },
            InstructionDetails { addressing: "Absolute",      assembler: "SBC oper",      opc: "ED", bytes: "3", cycles: "4"   },
            InstructionDetails { addressing: "Absolute,X",    assembler: "SBC oper,X",    opc: "FD", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "SBC oper,Y",    opc: "F9", bytes: "3", cycles: "4*"  },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "SBC (oper,X)",  opc: "E1", bytes: "2", cycles: "6"   },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "SBC (oper),Y",  opc: "F1", bytes: "2", cycles: "5*"  },
        ],
        exception: Some("* (+1 if page boundary crossed)"),
    });

    // --- STA / STX / STY ---
    m.insert("STA", ToolTip6502 {
        title: "STA",
        summary: Some("Store Accumulator"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "STA oper",      opc: "85", bytes: "2", cycles: "3" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "STA oper,X",    opc: "95", bytes: "2", cycles: "4" },
            InstructionDetails { addressing: "Absolute",      assembler: "STA oper",      opc: "8D", bytes: "3", cycles: "4" },
            InstructionDetails { addressing: "Absolute,X",    assembler: "STA oper,X",    opc: "9D", bytes: "3", cycles: "5" },
            InstructionDetails { addressing: "Absolute,Y",    assembler: "STA oper,Y",    opc: "99", bytes: "3", cycles: "5" },
            InstructionDetails { addressing: "(indirect,X)",  assembler: "STA (oper,X)",  opc: "81", bytes: "2", cycles: "6" },
            InstructionDetails { addressing: "(indirect),Y",  assembler: "STA (oper),Y",  opc: "91", bytes: "2", cycles: "6" },
        ],
        exception: None,
    });

    m.insert("STX", ToolTip6502 {
        title: "STX",
        summary: Some("Store X"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "STX oper",      opc: "86", bytes: "2", cycles: "3" },
            InstructionDetails { addressing: "zero Page,Y",   assembler: "STX oper,Y",    opc: "96", bytes: "2", cycles: "4" },
            InstructionDetails { addressing: "Absolute",      assembler: "STX oper",      opc: "8E", bytes: "3", cycles: "4" },
        ],
        exception: None,
    });

    m.insert("STY", ToolTip6502 {
        title: "STY",
        summary: Some("Store Y"),
        flags_note: Some("No flags changed."),
        rows: vec![
            InstructionDetails { addressing: "zero Page",     assembler: "STY oper",      opc: "84", bytes: "2", cycles: "3" },
            InstructionDetails { addressing: "zero Page,X",   assembler: "STY oper,X",    opc: "94", bytes: "2", cycles: "4" },
            InstructionDetails { addressing: "Absolute",      assembler: "STY oper",      opc: "8C", bytes: "3", cycles: "4" },
        ],
        exception: None,
    });

    // --- TAX / TAY / TSX / TXA / TXS / TYA ---
    m.insert("TAX", ToolTip6502 {
        title: "TAX", summary: Some("Transfer A → X"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TAX", opc: "AA", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("TAY", ToolTip6502 {
        title: "TAY", summary: Some("Transfer A → Y"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TAY", opc: "A8", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("TSX", ToolTip6502 {
        title: "TSX", summary: Some("Transfer SP → X"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TSX", opc: "BA", bytes: "1", cycles: "2" }
        ],
        exception: None,});

    m.insert("TXA", ToolTip6502 {
        title: "TXA", summary: Some("Transfer X → A"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TXA", opc: "8A", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("TXS", ToolTip6502 {
        title: "TXS", summary: Some("Transfer X → SP"),
        flags_note: Some("Flags unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TXS", opc: "9A", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m.insert("TYA", ToolTip6502 {
        title: "TYA", summary: Some("Transfer Y → A"),
        flags_note: Some("Affects N,Z; others unchanged."),
        rows: vec![ InstructionDetails { addressing: "implied", assembler: "TYA", opc: "98", bytes: "1", cycles: "2" }
        ],
        exception: None,
    });

    m
});