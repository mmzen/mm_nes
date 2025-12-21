DMA (OAM DMA + DMC DMA)
========

# Scope

You MUST implement:

OAM DMA triggered by writes to $4014, including exact cycle stealing and alignment behavior.

DMC DMA (load and reload fetches) including exact halt/dummy/align/read sequencing and cycle counts.

CPU stalling semantics: CPU repeats the halted read cycle during all DMA “no‑bus” cycles, and those repeated reads MUST produce real bus side effects.

DMC timing conflicts that can corrupt controller and PPUDATA reads, by virtue of (3).

APU register read activation edge cases during DMA via mixed address decoding.

OAM DRAM decay (including row refresh behavior and “only row at OAMADDR persists” when rendering is off too long).

# Fundamental timing model
## CPU cycles

The emulator MUST execute in single CPU cycles (not instruction steps). DMA is specified at CPU-cycle granularity.

## APU GET/PUT phase

Define a per‑CPU‑cycle phase bit:

phase = GET on one CPU cycle

phase = PUT on the next

It alternates every CPU cycle forever.

Define one APU cycle as two CPU cycles: GET then PUT.

## Power‑on alignment randomness (required)

On cold power‑up, the CPU’s starting alignment relative to the APU cycle is random: the first CPU cycle after reset release is randomly either GET or PUT.

On CPU reset, the divider alignment MUST NOT change (i.e., reset does not re-randomize or re-phase the GET/PUT alternation).

# External bus + “open bus” latch
## External bus signals

Each CPU cycle has a single external bus master that drives:

addr: u16

rw: Read|Write

data_out: u8 (only if Write)

data_in: u8 (only if Read)

## External open-bus latch

Maintain open_bus: u8 representing “last value driven on the external CPU data bus”.

Rules:

On any cycle where external hardware drives a read value onto the external bus (CPU read, DMA read), you MUST set open_bus = data_in.

On any cycle where the CPU or DMA drives data_out during a write, you MUST set open_bus = data_out (the bus is physically driven).

Readable APU internal registers that drive only the internal CPU bus (not the external bus) MUST NOT update open_bus. In particular: $4015 read (and test registers if present) do not affect external open bus.

# Global DMA arbitration and CPU stall semantics
## RDY stall semantics (required)

When DMA stalls the CPU:

The CPU core is held by deasserting RDY.

While RDY is deasserted, the 6502 core repeats the last read cycle indefinitely.

Therefore, during DMA “no‑bus” cycles, the external bus MUST show repeated reads from the halted address (and thus those reads MUST produce all normal side effects).

## Halt can only succeed on a CPU read cycle

For both DMA units:

A DMA “halt attempt” may be scheduled/initiated on any cycle.

The CPU is only actually halted when the current CPU cycle is a read.

If the current CPU cycle is a write, the halt attempt FAILS and the DMA unit MUST try again on the next cycle.

## Bus master selection per cycle

For each CPU cycle, choose the bus master in this order:

If DMC DMA is performing its actual memory read cycle this cycle → DMC is bus master.

Else if OAM DMA is performing an actual bus access this cycle

OAM GET memory read, or

OAM PUT write to $2004
→ OAM is bus master.

Else CPU is bus master (even if stalled; then it is the repeated-read master).

This priority rule is required because DMC DMA has priority over OAM DMA on GET cycles and can delay OAM DMA.

# OAM DMA ($4014) — required exact behavior
   4.1 Trigger

A CPU write to $4014 triggers OAM DMA.

Let page = data_out of that write.

## Start time

OAM DMA begins on the first CPU cycle after the $4014 write.

## Halt cycle

On each cycle starting immediately after the $4014 write:

OAM DMA attempts to halt the CPU.

The first cycle where the CPU is in a read and the halt succeeds is the OAM DMA halt cycle.

During the OAM DMA halt cycle, no DMA bus access occurs (no read, no write).

## Optional 1-cycle alignment (GET alignment)

After the halt cycle:

If the next CPU cycle is PUT, OAM DMA MUST perform exactly one alignment cycle (no DMA bus access) so that the first transfer read occurs on a GET cycle.

If the next CPU cycle is GET, no alignment cycle occurs.

## Transfer loop (256 bytes)

OAM DMA copies 256 bytes from CPU memory to PPU OAMDATA:

Initialize:

index = 0

latch: u8 undefined

Then repeat until index == 256:

GET cycle action (memory read)
If phase == GET AND OAM DMA’s next required operation is GET AND DMC DMA is not using the bus:

Read from CPU address: src = (page as u16) << 8 | index

latch = cpu_bus_read(src)

Update open_bus = latch.

After a successful GET read, set OAM DMA’s next required operation to PUT.

PUT cycle action (write to $2004)
If phase == PUT AND OAM DMA’s next required operation is PUT (and bus not taken by DMC, which never takes PUT):

Write latch to PPU register $2004 (OAMDATA), as if the CPU performed that write.

Update open_bus = latch.

Increment index += 1.
After a successful PUT write, set OAM DMA’s next required operation to GET.

If the current cycle phase does not match the next required operation (e.g., next is GET but phase is PUT), OAM DMA performs no bus access and simply waits.

This “next required operation” mechanism is mandatory to correctly handle DMC stealing GET cycles: when DMC steals a GET, OAM remains “waiting for GET”, so the following PUT becomes a no‑bus cycle, producing the documented realignment behavior.

## Total cycle cost

The CPU cycles stolen by OAM DMA (not counting the initiating $4014 write cycle) MUST be:

513 cycles if no alignment cycle,

514 cycles if an alignment cycle occurs.

This equals: 1 halt + (0 or 1 align) + 512 transfer cycles.

## OAM writes during rendering

Writes to OAM during rendering are ignored by the PPU. OAM DMA uses $2004 writes, so if OAM DMA occurs during rendering, the PPU must ignore those writes accordingly.

# DMC DMA — required exact behavior

This section defines only the DMA fetch behavior; you still must implement the DMC output unit and sample buffer consumption correctly enough to trigger reload DMAs at the right times.

## When DMC DMA is needed

Whenever:

sample_buffer_empty == true

bytes_remaining != 0

then the DMC memory reader MUST fetch one byte by stalling the CPU for 1–4 cycles (exact count determined by the DMA sequencing below), fill the sample buffer, increment/wrap the address, decrement bytes remaining, and potentially set IRQ / loop restart.

## Address and length side effects of the DMC fetch

On the actual DMA memory read cycle (the cycle where the fetch occurs):

Read from the current DMC address dmc_addr via CPU memory mapping hardware.

Store read byte into sample_buffer and mark it full.

Increment dmc_addr += 1; if it exceeds $FFFF, wrap to $8000.

Decrement bytes_remaining -= 1.

If bytes_remaining == 0 then:

If loop flag is set: restart sample (reload address and length from $4012/$4013-derived start), else:

If IRQ enabled flag is set: set DMC IRQ flag (IRQ line asserted until cleared).

## Load DMA vs Reload DMA scheduling

The DMC DMA engine has two distinct schedule origins:

### Reload DMA scheduling

Triggered when the output unit empties the sample buffer while bytes_remaining != 0.
The halt attempt MUST be scheduled on the next PUT cycle.

### Load DMA scheduling

Triggered when software enables the DMC via $4015 bit 4 and the sample buffer is empty.

The halt attempt MUST be scheduled on the GET cycle of the second APU cycle after that $4015 write.

(Interpretation is strict: count APU cycles as GET+PUT pairs; “second APU cycle after” means skip the remainder of the current APU cycle, then one full APU cycle, then schedule on the GET of the next.)

## Halt attempt behavior (all DMC DMAs)

Starting at the scheduled halt‑attempt cycle:

DMC DMA attempts to halt the CPU each CPU cycle.

The halt succeeds only on a CPU read cycle; if the CPU is writing, it fails and the DMA keeps trying next cycle.

The first successful halt cycle is called the DMC halt cycle.

## DMC DMA execution sequence (after a successful halt)

After the DMC halt cycle:

Dummy cycle: The immediately following CPU cycle is always a dummy cycle with no DMA bus access.

Optional alignment cycle: If the next cycle after dummy is not GET, perform exactly one alignment cycle with no DMA bus access.

DMA read cycle: On the next GET cycle, perform the actual memory read (Section 5.2).

Resume: CPU resumes immediately on the following cycle.

Total stolen cycles counted from the halt cycle inclusive:

3 cycles if no alignment (halt + dummy + read)

4 cycles if alignment is needed (halt + dummy + align + read)

## DMC DMA bugs (late RP2A03G behavior required)

You MUST emulate both of these bugs exactly as described below.

### Aborted DMA bug (explicit or implicit stop)

Condition:

Sample playback is stopped during the APU cycle before a reload DMA would schedule
(i.e., on the 2nd or 3rd CPU cycle before the would-be halt attempt).

Behavior:

A DMA sequence begins but is aborted after a single cycle:

Exactly one successful halt cycle occurs (CPU stalled for that one cycle).

There is no dummy, no alignment, and no memory read.

DMC address and counters are not modified (no fetch happened).

Cancellation rule:

If the halt would be delayed because the CPU is on a write cycle, the aborted DMA does not occur at all.

### Unexpected reload DMA bug (implicit stop only)

Condition:

Playback is stopped implicitly on the same APU cycle that a reload DMA would schedule
(i.e., on the 1st CPU cycle before the would-be halt attempt).

Behavior:

An extra reload DMA occurs (a second fetch), producing the sequence shown in the DMA article example (halt/dummy/(align)/read), and it may take 4 cycles when alignment is required.

# Register side effects during DMA (required)
## CPU repeated reads during DMA no-bus cycles

During any DMA no‑bus cycle (halt/dummy/align, plus any idle cycle where a DMA is active but not performing a bus access), the CPU must present the halted address and perform a real read of that address, including all read side effects.

This is mandatory for correctness of:

PPUDATA ($2007) auto-increment being applied multiple times

Controller ($4016/$4017) clocking behavior and DPCM “bit deletion” glitch

PPUSTATUS ($2002) vblank flag clear behavior, etc.

## DPCM controller glitch emergence must be natural from bus behavior

Controller CLK is derived from the address/read condition such that an address change for one cycle can create an extra rising edge and delete a bit.

Your implementation MUST produce this glitch solely via correct cycle-by-cycle bus mastering and repeated reads, not via ad-hoc hacks.

## APU register activation during DMA (mixed address decode)

You MUST implement the following internal decoding rule for APU register reads, because DMA can unintentionally trigger APU/joypad reads during no‑bus cycles and during DMA read cycles.

Define:

cpu_addr_6502 = the 6502 core’s held address during stall (the repeated read address A).

internal_addr_bus = the internal address bus selected by the chip mux:

during CPU bus ownership, it equals cpu_addr_6502

during DMC DMA read bus ownership, it equals dmc_addr

during OAM DMA GET read bus ownership, it equals oam_src_addr

during OAM DMA PUT write bus ownership, it equals $2004 (because that is the destination register address)

APU register read enable condition:

APU registers can only be read when bits [15:5] of cpu_addr_6502 match the $4000-$401F range.

When enabled during a read cycle:

The chip checks bits [4:0] of internal_addr_bus to decide what register is being accessed.

Effects:

If internal_addr_bus[4:0] == $16 or $17, the corresponding joypad is clocked.

If internal_addr_bus[4:0] == $15 (or CPU test pin asserted), the internal data bus is isolated from the external data bus and driven by the readable APU register value; this internal drive MUST NOT change external open_bus.

Bus conflict resolution rule (forced, deterministic):

If both an external device and the joypad circuitry drive the external bus on the same cycle (a “bus conflict”), the final data_in value MUST be computed as bitwise AND of the simultaneously-driven values (low dominates).

(You MUST use this deterministic rule to avoid leaving behavior undefined.)

# DMC DMA during OAM DMA (overlap) — required

You MUST allow DMC DMA to run during OAM DMA.

Rules:

While OAM DMA is active, the CPU is already stalled. DMC DMA may still schedule and run.

If DMC DMA’s memory read cycle occurs on a GET cycle where OAM DMA would otherwise perform its GET, DMC wins that cycle.

OAM DMA MUST use the “next required operation” mechanism in Section 4.5 so that losing a GET cycle forces OAM DMA to insert the appropriate waiting PUT cycle (realigning). This is required for correct overlap timing.

# OAM decay (dynamic RAM) — required exact model
   8.1 Physical basis you must model

OAM bits lose charge over time; reading restores charge.

Decay eventually puts the cell into an invalid state, and reading it resolves to a random valid bit.

OAM is accessed/refreshes in rows, not individual bytes.

On NTSC, OAM is refreshed during rendering and remains stable across a normal vblank, but if rendering is disabled for longer, OAM begins to decay.

If rendering is disabled for too long, only the row pointed to by OAMADDR persists.

# Row structure and refresh coupling

Define primary OAM as 256 bytes.

Define OAM rows:

There are 32 rows.

row_index = addr >> 3 (8 bytes per row).

A “row access” refreshes the entire row.

Additionally, emulate the documented coupling:

Accessing a byte of OAM refreshes an entire 9-byte row: 8 contiguous bytes of OAM1 plus 1 byte of OAM2.
(You MUST implement this coupling if you model OAM2/secondary OAM decay; at minimum, row refresh for OAM1 must behave as row-based.)

# OAM refresh sources (must be applied)

A row refresh occurs when any of the following happens:

CPU write to $2004 writes OAM and refreshes the row containing the destination OAMADDR.

CPU read from $2004 (if supported) refreshes the row containing the source OAMADDR.

OAM DMA PUT writes to $2004 refresh the written row.

During rendering, sprite evaluation accesses OAM such that effectively all of OAM is refreshed once per scanline.

When rendering is disabled for extended time, the PPU continuously accesses the address pointed by OAMADDR such that only that row stays refreshed.

# Retention timing (fixed constant, no ambiguity)

Define a PPU-time counter ppu_dot_time that increments once per PPU dot.

Define constant:

OAM_RETENTION_DOTS = 21 * 341 PPU dots.

This is the retention window used in this spec to match “stable across a standard NTSC vblank but not much longer”.

# Decay state machine (deterministic, required)

Maintain for each row:

last_refresh_dot: u64

decayed: bool

Rules evaluated continuously:

If ppu_dot_time - last_refresh_dot > OAM_RETENTION_DOTS, then set decayed = true for that row.

When a row with decayed == true is next accessed (read or write):

Before completing the access, resolve the entire row’s 8 bytes to a new pseudo-random row value:

row_bytes[i] = PRNG.next_u8() for i=0..7

Set decayed = false

Set last_refresh_dot = ppu_dot_time

Then perform the actual read/write on top of that state (writes overwrite the targeted byte as normal).

This matches: decay leads to an invalid state, and reading restores charge while producing a random result.

# Rendering-disabled persistence rule

If both background and sprite rendering are disabled (PPUMASK), then on every PPU dot you MUST refresh the row containing the current OAMADDR:

row_index = OAMADDR >> 3

Set that row’s last_refresh_dot = ppu_dot_time continuously.

This enforces: only the row pointed to by OAMADDR persists when rendering is off too long.

# Implementation contract (what your core must expose)

The DMA subsystem MUST operate with these inputs per CPU cycle:

cpu_cycle_is_read: bool (current CPU bus direction)

cpu_addr_6502: u16 (current/held address)

phase: GET|PUT

ppu_dot_time: u64 (for OAM decay timing)

access to cpu_bus_read(addr)->u8 and cpu_bus_write(addr,u8)

access to ppu_write_2004(u8) and ppu_read_2004()->u8 (with correct PPU-side behavior)

DMC state (sample_buffer_empty, bytes_remaining, dmc_addr, loop/irq flags, etc.)

The DMA subsystem MUST output per cycle:

whether it asserts RDY (stall)

whether it owns the bus and which address/data it drives

the single-cycle bus action to execute (read/write/no-op)

# Non-negotiable acceptance criteria

Your implementation is considered correct only if:

OAM DMA consumes exactly 513 or 514 CPU cycles as defined, with correct GET/PUT alignment.

DMC DMA consumes exactly 3 or 4 cycles from halt to completion depending on alignment, and halting is delayed by CPU write cycles.

During DMA no-bus cycles, the CPU performs repeated reads that trigger real side effects (PPUDATA increments multiple times; controller glitch emerges naturally).

DMC DMA has priority over OAM DMA on GET, and overlap produces realignment stalls.

APU register activation behavior uses the mixed decode rule (6502 [15:5] gating + internal [4:0] selection), and readable APU regs do not update external open bus.

OAM decay is row-based, time-based, refreshed by access, and “only row at OAMADDR persists” when rendering disabled long enough.