use once_cell::sync::Lazy;
use std::collections::HashMap;
use crate::tooltip::ToolTip;

pub struct ToolTip6502 {}

impl ToolTip for ToolTip6502 {
    fn tooltip(mnemonic: &str) -> Option<&'static str> {
        TOOLTIP_MAP.get(mnemonic).copied()
    }
}


pub static TOOLTIP_MAP: Lazy<HashMap<&'static str, &'static str>> = Lazy::new(|| {
    let mut m = HashMap::new();
    
    m.insert("ADC", r#"ADC

    Add with Carry (A ← A + M + C), affects N,Z,C,V

    Flags: N,Z,C,V set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	ADC #oper	69	2	2
    zero page	ADC oper	65	2	3
    zero page,X	ADC oper,X	75	2	4
    absolute	ADC oper	6D	3	4
    absolute,X	ADC oper,X	7D	3	4*
    absolute,Y	ADC oper,Y	79	3	4*
    (indirect,X)	ADC (oper,X)	61	2	6
    (indirect),Y	ADC (oper),Y	71	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("AND", r#"AND

    Logical AND (A ← A & M), affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	AND #oper	29	2	2
    zero page	AND oper	25	2	3
    zero page,X	AND oper,X	35	2	4
    absolute	AND oper	2D	3	4
    absolute,X	AND oper,X	3D	3	4*
    absolute,Y	AND oper,Y	39	3	4*
    (indirect,X)	AND (oper,X)	21	2	6
    (indirect),Y	AND (oper),Y	31	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("ASL", r#"ASL

    Arithmetic Shift Left (C ← b7; A/M ← A/M<<1), affects N,Z,C

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    accumulator	ASL A	0A	1	2
    zero page	ASL oper	06	2	5
    zero page,X	ASL oper,X	16	2	6
    absolute	ASL oper	0E	3	6
    absolute,X	ASL oper,X	1E	3	7
"#);
    m.insert("BCC", r#"BCC

    Branch if Carry Clear (C=0)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BCC oper	90	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BCS", r#"BCS

    Branch if Carry Set (C=1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BCS oper	B0	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BEQ", r#"BEQ

    Branch if Zero Set (Z=1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BEQ oper	F0	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BIT", r#"BIT

    Bit Test (Z ← A&M==0; N ← M7; V ← M6)

    Flags: Z,N,V set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	BIT oper	24	2	3
    absolute	BIT oper	2C	3	4
"#);
    m.insert("BMI", r#"BMI

    Branch if Negative Set (N=1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BMI oper	30	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BNE", r#"BNE

    Branch if Zero Clear (Z=0)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BNE oper	D0	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BPL", r#"BPL

    Branch if Negative Clear (N=0)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BPL oper	10	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BRK", r#"BRK

    Force Interrupt (push PC+2,P; set I; jump via IRQ/BRK vector)

    Flags: I set; B used on stack.
    addressing	assembler	opc	bytes	cycles
    implied	BRK	00	1	7
"#);
    m.insert("BVC", r#"BVC

    Branch if Overflow Clear (V=0)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BVC oper	50	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("BVS", r#"BVS

    Branch if Overflow Set (V=1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    relative	BVS oper	70	2	2 (+1 if branch taken, +1 if page boundary crossed)
"#);
    m.insert("CLC", r#"CLC

    Clear Carry flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	CLC	18	1	2
"#);
    m.insert("CLD", r#"CLD

    Clear Decimal flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	CLD	D8	1	2
"#);
    m.insert("CLI", r#"CLI

    Clear Interrupt Disable flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	CLI	58	1	2
"#);
    m.insert("CLV", r#"CLV

    Clear Overflow flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	CLV	B8	1	2
"#);
    m.insert("CMP", r#"CMP

    Compare A with memory (sets Z,N,C like subtraction)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	CMP #oper	C9	2	2
    zero page	CMP oper	C5	2	3
    zero page,X	CMP oper,X	D5	2	4
    absolute	CMP oper	CD	3	4
    absolute,X	CMP oper,X	DD	3	4*
    absolute,Y	CMP oper,Y	D9	3	4*
    (indirect,X)	CMP (oper,X)	C1	2	6
    (indirect),Y	CMP (oper),Y	D1	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("CPX", r#"CPX

    Compare X with memory (sets Z,N,C like subtraction)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	CPX #oper	E0	2	2
    zero page	CPX oper	E4	2	3
    absolute	CPX oper	EC	3	4
"#);
    m.insert("CPY", r#"CPY

    Compare Y with memory (sets Z,N,C like subtraction)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	CPY #oper	C0	2	2
    zero page	CPY oper	C4	2	3
    absolute	CPY oper	CC	3	4
"#);
    m.insert("DEC", r#"DEC

    Decrement memory by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	DEC oper	C6	2	5
    zero page,X	DEC oper,X	D6	2	6
    absolute	DEC oper	CE	3	6
    absolute,X	DEC oper,X	DE	3	7
"#);
    m.insert("DEX", r#"DEX

    Decrement X by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	DEX	CA	1	2
"#);
    m.insert("DEY", r#"DEY

    Decrement Y by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	DEY	88	1	2
"#);
    m.insert("EOR", r#"EOR

    Exclusive OR (A ← A ^ M), affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	EOR #oper	49	2	2
    zero page	EOR oper	45	2	3
    zero page,X	EOR oper,X	55	2	4
    absolute	EOR oper	4D	3	4
    absolute,X	EOR oper,X	5D	3	4*
    absolute,Y	EOR oper,Y	59	3	4*
    (indirect,X)	EOR (oper,X)	41	2	6
    (indirect),Y	EOR (oper),Y	51	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("INC", r#"INC

    Increment memory by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	INC oper	E6	2	5
    zero page,X	INC oper,X	F6	2	6
    absolute	INC oper	EE	3	6
    absolute,X	INC oper,X	FE	3	7
"#);
    m.insert("INX", r#"INX

    Increment X by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	INX	E8	1	2
"#);
    m.insert("INY", r#"INY

    Increment Y by one

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	INY	C8	1	2
"#);
    m.insert("JMP", r#"JMP

    Jump to address (absolute or indirect)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    absolute	JMP oper	4C	3	3
    indirect	JMP (oper)	6C	3	5
"#);
    m.insert("JSR", r#"JSR

    Jump to Subroutine (push return-1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    absolute	JSR oper	20	3	6
"#);
    m.insert("LDA", r#"LDA

    Load Accumulator with memory; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	LDA #oper	A9	2	2
    zero page	LDA oper	A5	2	3
    zero page,X	LDA oper,X	B5	2	4
    absolute	LDA oper	AD	3	4
    absolute,X	LDA oper,X	BD	3	4*
    absolute,Y	LDA oper,Y	B9	3	4*
    (indirect,X)	LDA (oper,X)	A1	2	6
    (indirect),Y	LDA (oper),Y	B1	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("LDX", r#"LDX

    Load X with memory; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	LDX #oper	A2	2	2
    zero page	LDX oper	A6	2	3
    zero page,Y	LDX oper,Y	B6	2	4
    absolute	LDX oper	AE	3	4
    absolute,Y	LDX oper,Y	BE	3	4*

  * * (+1 if page boundary crossed)
"#);
    m.insert("LDY", r#"LDY

    Load Y with memory; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	LDY #oper	A0	2	2
    zero page	LDY oper	A4	2	3
    zero page,X	LDY oper,X	B4	2	4
    absolute	LDY oper	AC	3	4
    absolute,X	LDY oper,X	BC	3	4*

  * * (+1 if page boundary crossed)
"#);
    m.insert("LSR", r#"LSR

    Logical Shift Right (C ← b0; A/M ← A/M>>1; bit7←0)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    accumulator	LSR A	4A	1	2
    zero page	LSR oper	46	2	5
    zero page,X	LSR oper,X	56	2	6
    absolute	LSR oper	4E	3	6
    absolute,X	LSR oper,X	5E	3	7
"#);
    m.insert("NOP", r#"NOP

    No Operation

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	NOP	EA	1	2
"#);
    m.insert("ORA", r#"ORA

    Logical Inclusive OR (A ← A | M), affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	ORA #oper	09	2	2
    zero page	ORA oper	05	2	3
    zero page,X	ORA oper,X	15	2	4
    absolute	ORA oper	0D	3	4
    absolute,X	ORA oper,X	1D	3	4*
    absolute,Y	ORA oper,Y	19	3	4*
    (indirect,X)	ORA (oper,X)	01	2	6
    (indirect),Y	ORA (oper),Y	11	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("PHA", r#"PHA

    Push Accumulator on stack

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	PHA	48	1	3
"#);
    m.insert("PHP", r#"PHP

    Push Processor Status on stack (B flag set in pushed value)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	PHP	08	1	3
"#);
    m.insert("PLA", r#"PLA

    Pull Accumulator from stack; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	PLA	68	1	4
"#);
    m.insert("PLP", r#"PLP

    Pull Processor Status from stack

    Flags: P restored.
    addressing	assembler	opc	bytes	cycles
    implied	PLP	28	1	4
"#);
    m.insert("ROL", r#"ROL

    Rotate Left through Carry (C ↔ b0; A/M ← (A/M<<1)|C)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    accumulator	ROL A	2A	1	2
    zero page	ROL oper	26	2	5
    zero page,X	ROL oper,X	36	2	6
    absolute	ROL oper	2E	3	6
    absolute,X	ROL oper,X	3E	3	7
"#);
    m.insert("ROR", r#"ROR

    Rotate Right through Carry (C ↔ b7; A/M ← (A/M>>1)|C<<7)

    Flags: N,Z,C set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    accumulator	ROR A	6A	1	2
    zero page	ROR oper	66	2	5
    zero page,X	ROR oper,X	76	2	6
    absolute	ROR oper	6E	3	6
    absolute,X	ROR oper,X	7E	3	7
"#);
    m.insert("RTI", r#"RTI

    Return from Interrupt (pull P then PC)

    Flags: P restored.
    addressing	assembler	opc	bytes	cycles
    implied	RTI	40	1	6
"#);
    m.insert("RTS", r#"RTS

    Return from Subroutine (pull PC then PC←PC+1)

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	RTS	60	1	6
"#);
    m.insert("SBC", r#"SBC

    Subtract with Carry (A ← A - M - (1-C)), affects N,Z,C,V

    Flags: N,Z,C,V set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    immediate	SBC #oper	E9	2	2
    zero page	SBC oper	E5	2	3
    zero page,X	SBC oper,X	F5	2	4
    absolute	SBC oper	ED	3	4
    absolute,X	SBC oper,X	FD	3	4*
    absolute,Y	SBC oper,Y	F9	3	4*
    (indirect,X)	SBC (oper,X)	E1	2	6
    (indirect),Y	SBC (oper),Y	F1	2	5*

  * * (+1 if page boundary crossed)
"#);
    m.insert("SEC", r#"SEC

    Set Carry flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	SEC	38	1	2
"#);
    m.insert("SED", r#"SED

    Set Decimal flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	SED	F8	1	2
"#);
    m.insert("SEI", r#"SEI

    Set Interrupt Disable flag

    Flags: only specified flag affected.
    addressing	assembler	opc	bytes	cycles
    implied	SEI	78	1	2
"#);
    m.insert("STA", r#"STA

    Store Accumulator to memory

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	STA oper	85	2	3
    zero page,X	STA oper,X	95	2	4
    absolute	STA oper	8D	3	4
    absolute,X	STA oper,X	9D	3	5
    absolute,Y	STA oper,Y	99	3	5
    (indirect,X)	STA (oper,X)	81	2	6
    (indirect),Y	STA (oper),Y	91	2	6
"#);
    m.insert("STX", r#"STX

    Store X to memory

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	STX oper	86	2	3
    zero page,Y	STX oper,Y	96	2	4
    absolute	STX oper	8E	3	4
"#);
    m.insert("STY", r#"STY

    Store Y to memory

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    zero page	STY oper	84	2	3
    zero page,X	STY oper,X	94	2	4
    absolute	STY oper	8C	3	4
"#);
    m.insert("TAX", r#"TAX

    Transfer A → X; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TAX	AA	1	2
"#);
    m.insert("TAY", r#"TAY

    Transfer A → Y; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TAY	A8	1	2
"#);
    m.insert("TSX", r#"TSX

    Transfer SP → X; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TSX	BA	1	2
"#);
    m.insert("TXA", r#"TXA

    Transfer X → A; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TXA	8A	1	2
"#);
    m.insert("TXS", r#"TXS

    Transfer X → SP

    Flags: unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TXS	9A	1	2
"#);
    m.insert("TYA", r#"TYA

    Transfer Y → A; affects N,Z

    Flags: N,Z set; others unchanged.
    addressing	assembler	opc	bytes	cycles
    implied	TYA	98	1	2
"#);

    m
});