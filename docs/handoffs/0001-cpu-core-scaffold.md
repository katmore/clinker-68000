# Handoff 0001 — CPU core scaffold

One-time handoff so a fresh session can resume cold. Written 2026-08-31.
**Superseded once acted on** — don't keep it current; write `0002-*.md` for the
next handoff.

Read these first, in order: this file → `docs/ARCHITECTURE.md` →
`docs/CLINKER-68000-SPEC.md` §3.1 and §10.

---

## 1. TL;DR

Cargo workspace + a **skeleton** MC68000 core, committed and pushed. The CPU can
reset, single-step, and vector exceptions; the decoder knows ~7 instructions, so
it is **not a working CPU**. The address map is decided and implemented, with no
real peripherals behind it.

- `. "$HOME/.cargo/env"` then `cargo test` → **11 passing**.
  `cargo clippy --all-targets` → clean. `cargo fmt --all` → no diff.
- All work is on `main` and pushed. Run `git status` / `git log --oneline` to see
  where things stand (last handoff commit was `c0c39be`; the user also pushes
  small README edits directly, sometimes from GitHub's web UI).
- Remote: `git@github.com:katmore/clinker-68000.git` (SSH).

## 2. Environment gotcha (will bite you)

Rust was installed this session via rustup into `~/.cargo`, which is **not on
PATH**. Every shell must first:

```
. "$HOME/.cargo/env"
```

`rust-toolchain.toml` pins `stable` + `clippy` + `rustfmt`; `cargo` will
auto-provision the toolchain, but rustup itself must exist and the env must be
sourced. See the `rust-toolchain-setup` memory.

There is **no binary and no `main.rs`** — it's libraries + tests. Exercise the
core with `cargo test`, a throwaway `#[test]`, or `Machine::run()`. No CI.

## 3. What exists

Workspace = two crates under `crates/`. `edition = "2021"`, `resolver = "2"`,
`rust-version = "1.98"`. `Cargo.lock` is committed on purpose (a binary/FFI crate
is coming).

### `clinker-m68k` — the CPU, knows nothing about this machine

| File | State |
|---|---|
| `src/lib.rs` | Re-exports. `CLOCK_HZ = 8_000_000`, `ADDRESS_MASK = 0x00FF_FFFF`. `#![forbid(unsafe_code)]`. |
| `src/bus.rs` | `Bus` trait. Required: `read_u8`/`write_u8`. Provided: `read/write_u16/u32` (big-endian; odd addr → `BusError::Address`). Clock-driven hooks with **no-op defaults**: `advance(cycles)` (peripheral time tick, called *through* the instruction) and `interrupt_level() -> u8` (sampled each instruction boundary). `Width::bus_cycles()` = 4 / 4 / 8. `BusError::{Unmapped(addr), Address(addr)}`. |
| `src/registers.rs` | `Registers`: `d[8]`, `a[7]` (A0–A6), `usp`, `ssp`, `pc`, `sr`. A7 banking via `sp()`/`set_sp()`/`addr(i)`/`set_addr(i)` keyed on the S bit. `StatusRegister(u16)` with `TRACE`/`SUPERVISOR`/`INT_MASK` consts, `RESET` const, `supervisor()`/`trace()`/`interrupt_mask()`/`contains(cc)`/`set(cc,bool)`/`ccr()`. `ConditionCode` enum. |
| `src/decode.rs` | **STUB.** `decode(u16) -> Decoded`. Recognises `NOP RESET RTS TRAP#n MOVEQ BRA.s ILLEGAL` + line-A/F ranges; all else → `Decoded::Unimplemented(op)`. `BRA.w`/`BRA.l` (disp byte == 0) explicitly → `Unimplemented`. **No extension-word reads anywhere yet.** Unit tests. |
| `src/execute.rs` | `execute(cpu, bus, Decoded) -> Result<u32, Exception>`. Returns *internal* (non-bus) cycles only; bus cycles are charged by the `Cpu::mem_*` helpers. `RESET` checks supervisor → else `PrivilegeViolation`. |
| `src/exception.rs` | `Exception` enum → `vector()` / `vector_offset()`. `VECTOR_TABLE_BYTES = 1024`. Autovector interrupts = `24 + level`. |
| `src/cpu.rs` | `Cpu { regs, cycles: u64, step_cycles: u32, state }`. `RunState::{Running, Stopped, Halted}` (`Stopped` currently unreachable). `reset(bus)`: forces `SR = RESET`, reads SSP/PC from `$0`/`$4` (raw, not ticked), bad read → `Halted`. `step(bus)`: interrupt sample → `fetch_opcode` → `decode` → `execute` → on `Err` `enter_exception`. `tick()` bumps `step_cycles` **and** calls `bus.advance()`. Ticked `mem_read/write_u16/u32` shared with `execute` (`pub(crate)`). `enter_exception`: pushes the **6-byte** frame (PC long, SR word) to SSP, enters supervisor, clears T, raises mask for interrupts, loads handler from the vector; a fault mid-push → `Halted`. |

### `clinker-core` — the machine, owns address decode

| File | State |
|---|---|
| `src/lib.rs` | Front-end boundary notes (the 5 ops the GTK repo will need). Re-exports `Machine`, `SystemBus`, and map consts `IO_BASE RAM_BYTES RAM_END ROM_BASE ROM_BYTES SYSCTL_ADDR`. |
| `src/system_bus.rs` | `SystemBus { ram: 16 MiB, rom: 128 KiB, reset_overlay: bool }` impls `Bus`. `decode()` → RAM `<$FC0000` / ROM `$FC0000..$FE0000` / Display `$FE0000..$FF0000` (inert, reads `$FF`, writes dropped) / IO `$FF0000..$1000000`. Reset overlay: while active, reads of `$0..$400` come from ROM's low 1 KiB; writes there fall through to RAM underneath. Overlay cleared by writing a byte with bit 0 = 0 to `SYSCTL_ADDR` (`$FF0000`). `read_io(0)` returns the overlay bit; all other IO reads `$FF`. `Target::Unmapped` is currently **unreachable** (map covers the whole 24-bit space) — kept as a defensive branch. `load_rom` / `load_ram` helpers. Unit tests. |
| `src/machine.rs` | `Machine { cpu, bus }` + `load_rom` / `load_ram` / `reset` / `step` / `run(max_steps)`. Integration tests build a boot ROM and drive it. |

## 4. Decisions locked — do not relitigate

Recorded in `docs/CLINKER-68000-SPEC.md` (§3.1, §10.1–§10.4, Open Questions) and
`docs/ARCHITECTURE.md`.

1. **Timing:** clock-driven core, target = accurate per-instruction cycle
   *totals* (yacht.txt / Motorola tables). **No prefetch-queue modelling.** Full
   cycle-exact stays a possible additive upgrade, not planned.
2. **Address map:** full 16 MB RAM populated, I/O + ROM + display in the top
   256 KB, reset ROM overlay at `$0`. Table in spec §3.1.
3. **Exceptions:** MC68000 (not 010/020) → 6-byte frame for groups 1 & 2.
   Group-0 (bus/address error) gets a *diagnostic-grade* 14-byte frame when
   built; **no MMU → bus error is fatal, instruction restart is not a goal.**
4. **License:** `GPL-3.0-or-later` (Cargo tag + README notice agree). ©2026 D.B.
5. **Crate split:** CPU talks only to `Bus`; machine owns decode. FFI crate
   later. GTK front end = separate repo.
6. **Scope:** this repo is **68000-only**. `clinker-6502` is its own repo
   (`katmore/clinker-6502`), handled in another session.

## 5. Known inaccuracies / shortcuts in the current code

- **Cycle counts are placeholder**, not yacht.txt-validated: `reset` = 16,
  `RESET` insn internal = 128, `RTS` internal = 4, `BRA.s` = 6,
  `enter_exception` = 18 (+ the 3 bus cycles it charges) or 8 on halt. Treat all
  of these as "shape is right, numbers are eyeballed" — fix during the decoder
  pass.
- **Interrupts:** `step` takes `Exception::Interrupt(level)` when
  `level == 7 || level > SR mask`, vectors via autovector `24+level`, and raises
  the SR mask to `level`. No IACK bus cycle; autovector always assumed. Entirely
  **untested** (no source drives `interrupt_level()` above 0 yet).
- **`enter_exception` pushes the 6-byte frame for _everything_**, including
  bus/address error (which need the 14-byte frame).
- **No extension-word handling** in fetch or decode. The real decoder needs a
  fetch path that pulls extension words (and, eventually, a prefetch model).
- Address masking to 24 bits happens both in `Cpu::mem_*` and again in
  `SystemBus::decode` (deliberate belt-and-suspenders).
- `reset()` uses raw `bus.read_u32`, not the ticked helper (it's not a "step").

## 6. Test coverage — thin, here's what's NOT covered

Covered: reset-through-overlay, NOP, MOVEQ (non-zero value only), ILLEGAL
vectoring + frame push, BRA.s self-loop, NOP cycle accounting, overlay
read/write/clear, ROM read-only, inert display/IO.

**Not covered:** `RTS`, `TRAP`, `RESET` (both privilege paths), MOVEQ N/Z flags,
the interrupt path, double-fault → `Halted`, `reset()` → `Halted` on bad vectors,
address error on odd PC, big-endian correctness of `mem_write_u32`, `run()`
stopping conditions.

## 7. Next steps (priority order)

1. **Real instruction decoder + execute.**
   - Decide the dispatch style: generated 64K jump table (Musashi/Moira) vs
     match-arm masking (what the stub does, scales poorly past ~50 opcodes).
   - Build a proper effective-address layer (all 12 modes + extension words).
   - Implement the core integer set: `MOVE(A)`, ALU (`ADD/SUB/AND/OR/EOR/CMP` +
     immediates + `ADDA/SUBA/CMPA`), `MOVEQ/ADDQ/SUBQ`, shifts/rotates, `Bcc`
     with .w displacement, `BRA/BSR`, `JMP/JSR/RTS/RTR`, `LEA/PEA`, `Scc`,
     `MOVEM`, `EXT`, `SWAP`, `CLR/NEG/NOT/TST`, `LINK/UNLK`, `MOVE to/from SR/CCR/USP`,
     `ANDI/ORI/EORI to CCR/SR`, `TRAP/TRAPV/CHK`, `STOP`, `RESET`, `NOP`.
   - Validate against **SingleStepTests/ProcessorTests** (formerly
     TomHarte/ProcessorTests), `680x0/` — JSON, ~8k cases/opcode with
     initial+final register/RAM state and per-cycle bus transactions. Final
     state first; wire the per-cycle bus comparison once `advance` granularity is
     tightened.
   - Fill in real cycle counts per instruction as you go (§5).
2. **Group-0 stack frame.** Add the 14-byte bus/address-error frame in
   `enter_exception` (fault addr + R/W + FC bits accurate; internal-status bits
   zeroed; documented diagnostic-grade).
3. **`STOP` / `TRACE`.** Make `RunState::Stopped` reachable; act on the T bit
   (trace exception after each instruction when T set).
4. **Exception priority / simultaneity** — implement when the first interrupt
   source lands, not before.
5. **First peripheral** (probably the CRTC text buffer) — needs the I/O register
   layout decided first (§8). Pattern: peripherals become fields on `SystemBus`,
   routed in `decode()`; `advance()` fans out to them; `interrupt_level()`
   aggregates their IRQ lines.

**Milestone-2 "done":** passes ProcessorTests for the integer set above, and can
run a hand-assembled boot stub that clears the overlay, sets SP, copies the
vector table to RAM, and enters a loop.

## 8. Blocked on user input (see spec "Open Questions")

- **Boot ROM size / contents** — map provisionally reserves 128 KB at `$FC0000`.
  Depends on whether the primary language/OS is ROM-resident or floppy-loaded.
  Doesn't block the decoder; blocks finalising the map and writing real firmware.
- **I/O register layout** — `$FF0000` page is one undivided block; per-chip
  offsets (FDC + drive-select expander, NCR 5380, 68681 DUART, Centronics,
  6850 ACIA ×2, OPL2, keyboard, system-control) unassigned. Blocks the first
  peripheral.
- **System-control register** — confirm address + bit for the overlay-clear bit
  (code assumes `$FF0000` bit 0).

## 9. Memory / context notes

A fresh Claude Code session auto-loads the project memory index. Relevant files:
`project-direction` (this vs the 6502 repo), `rust-toolchain-setup` (the PATH
gotcha), and `clinker-lore-context` — **background only, must never appear in the
repo, commits, or code comments.**
