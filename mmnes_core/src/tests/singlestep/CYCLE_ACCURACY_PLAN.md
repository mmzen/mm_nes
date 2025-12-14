# Cycle Accuracy Fix Plan for SingleStepTests

## Current Status
- **729,191 passed / 1,830,809 failed (28.5% pass rate)**
- 49 opcodes fully pass (immediate, zeropage, absolute addressing mostly work)
- Main issues: missing dummy reads/writes for cycle accuracy

## Issue Categories & Fixes

### Priority 1: Implied/Accumulator Mode Dummy Reads (Easy - ~30 opcodes)

**Problem:** 2-cycle instructions only do 1 bus read (opcode fetch)
**Solution:** Add dummy read of PC+1 after opcode fetch for implied mode

**Affected opcodes:**
- Flag ops: CLC, SEC, CLI, SEI, CLV, CLD, SED (0x18, 0x38, 0x58, 0x78, 0xB8, 0xD8, 0xF8)
- Transfers: TAX, TAY, TXA, TYA, TSX, TXS (0xAA, 0xA8, 0x8A, 0x98, 0xBA, 0x9A)
- Inc/Dec: INX, INY, DEX, DEY (0xE8, 0xC8, 0xCA, 0x88)
- NOP: 0xEA
- Accumulator shifts: ASL A, LSR A, ROL A, ROR A (0x0A, 0x4A, 0x2A, 0x6A)

**Fix location:** `fetch_operand()` for `AddressingMode::Implicit` and `AddressingMode::Accumulator`

```rust
AddressingMode::Implicit => {
    // Dummy read of next byte (discarded)
    let _ = bus.borrow().read_byte(registers.safe_pc_add(1)?)?;
    Operand::None
},
AddressingMode::Accumulator => {
    // Dummy read of next byte (discarded)
    let _ = bus.borrow().read_byte(registers.safe_pc_add(1)?)?;
    Operand::Accumulator
},
```

### Priority 2: Read-Modify-Write Double Write (Medium - ~20 opcodes)

**Problem:** RMW instructions do [read, write] but should do [read, write_old, write_new]
**Solution:** In RMW instruction execution, write old value before new value

**Affected opcodes:**
- ASL: 0x06, 0x0E, 0x16, 0x1E
- LSR: 0x46, 0x4E, 0x56, 0x5E
- ROL: 0x26, 0x2E, 0x36, 0x3E
- ROR: 0x66, 0x6E, 0x76, 0x7E
- INC: 0xE6, 0xEE, 0xF6, 0xFE
- DEC: 0xC6, 0xCE, 0xD6, 0xDE
- Illegal RMW: SLO, SRE, RLA, RRA, ISB, DCP

**Fix location:** Each RMW instruction's execute function

```rust
// Example for ASL zeropage
fn asl_zeropage(cpu: &mut Cpu6502, operand: &Operand) -> Result<u32, CpuError> {
    let addr = operand.address()?;
    let old_value = cpu.bus.borrow().read_byte(addr)?;

    // Dummy write of old value (cycle 4)
    cpu.bus.borrow_mut().write_byte(addr, old_value)?;

    // Actual write of new value (cycle 5)
    let new_value = old_value << 1;
    cpu.bus.borrow_mut().write_byte(addr, new_value)?;

    // Set flags...
}
```

### Priority 3: Indexed Addressing Dummy Reads (Medium - ~40 opcodes)

**Problem:** Missing intermediate bus cycles for indexed modes

#### 3a. Zero Page Indexed (zpx, zpy)
Need to read from unindexed address before indexed address.

**Fix:**
```rust
AddressingMode::ZeroPageIndexedX => {
    let base = bus.borrow().read_byte(registers.safe_pc_add(1)?)?;
    // Dummy read from base address (before adding X)
    let _ = bus.borrow().read_byte(base as u16)?;
    let effective = base.wrapping_add(registers.x);
    Operand::Address(effective as u16)
}
```

#### 3b. Absolute Indexed (absx, absy) - Page Crossing
Need dummy read when page boundary NOT crossed for certain instructions.

**Pattern:**
- Read instructions: Extra cycle only when page crossed (already working)
- Write instructions: Always 5 cycles (dummy read of wrong address)
- RMW instructions: Always 7 cycles (dummy read of wrong address)

### Priority 4: Branch Page Crossing (Easy - 8 opcodes)

**Problem:** When branch crosses page, need extra cycle with dummy read

**Affected:** BPL, BMI, BVC, BVS, BCC, BCS, BNE, BEQ

**Fix:** In branch execution, when page crossed:
```rust
if page_crossed {
    // Dummy read from wrong address (same page as branch instruction)
    let wrong_addr = (target & 0x00FF) | (old_pc & 0xFF00);
    let _ = bus.borrow().read_byte(wrong_addr)?;
}
```

### Priority 5: Stack Operations (Medium - 6 opcodes)

**Problem:** Stack ops missing internal operation cycles

**JSR (0x20):** 6 cycles
1. Fetch opcode
2. Fetch low byte of address
3. Internal operation (read stack pointer location - dummy)
4. Push PCH
5. Push PCL
6. Fetch high byte of address

**RTS (0x60):** 6 cycles
1. Fetch opcode
2. Dummy read of next byte
3. Dummy read of stack pointer location
4. Pull PCL
5. Pull PCH
6. Dummy read of PC (increment)

**RTI (0x40):** 6 cycles - similar pattern

**PHA/PHP (0x48, 0x08):** 3 cycles
1. Fetch opcode
2. Dummy read of next byte
3. Push value

**PLA/PLP (0x68, 0x28):** 4 cycles
1. Fetch opcode
2. Dummy read of next byte
3. Dummy read of stack pointer (increment)
4. Pull value

### Priority 6: BRK (0x00) - 7 cycles

**Current issue:** Missing cycles, wrong order

**Correct sequence:**
1. Fetch opcode (BRK)
2. Fetch operand (padding byte, discarded but PC increments)
3. Push PCH
4. Push PCL
5. Push P (with B flag set)
6. Fetch vector low from $FFFE
7. Fetch vector high from $FFFF

### Priority 7: Indirect Indexed Modes

**Indirect X (izx):** 6 cycles for read, needs dummy read of base
**Indirect Y (izy):** 5-6 cycles, needs page crossing handling

## Implementation Order

1. **Phase 1:** Implied/Accumulator dummy reads (biggest bang for buck)
2. **Phase 2:** RMW double writes
3. **Phase 3:** Branch page crossing
4. **Phase 4:** Stack operations
5. **Phase 5:** Indexed addressing refinements
6. **Phase 6:** BRK and indirect modes

## Architecture Decision

**Option A:** Modify existing instruction implementations
- Pro: Minimal code changes
- Con: Scattered changes, hard to maintain

**Option B:** Create cycle-accurate execution mode
- Add bus cycle tracking to CPU
- Execute instructions cycle-by-cycle
- Pro: Clean separation, accurate
- Con: Major refactor

**Recommendation:** Start with Option A for quick wins, consider Option B for complex cases.

## Verification

After each fix:
```bash
cargo test test_opcode_XX -- --ignored --nocapture
```

Track progress:
```bash
cargo test test_all_opcodes -- --ignored --nocapture 2>&1 | grep "TOTAL:"
```
