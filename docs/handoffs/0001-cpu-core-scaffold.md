# Handoff 0001 — CPU core scaffold

One-time handoff so a fresh session can resume cold. Written 2026-08-31.
Superseded once acted on — no need to keep it current.

## TL;DR

Cargo workspace + a **skeleton** MC68000 core are in place and committed. The
CPU can reset, single-step, and vector exceptions, but the decoder only knows ~7
instructions — it is not a working CPU. Address map is decided and implemented as
a flat decoder with no real peripherals. Next real work is the full instruction
decoder, validated against TomHarte vectors.

Everything is committed and pushed; `main` == `origin/main` at `6e1c2cb`.
`cargo test` = 11 passing, `cargo clippy --all-targets` clean.

## Build / test (IMPORTANT gotcha)

Rust was installed this session via rustup to `~/.cargo`, which is **not on
PATH**. Every shell must first:

```
. "$HOME/.cargo/env"
```

Then `cargo build` / `cargo test` / `cargo clippy --all-targets` / `cargo fmt --all`
from the repo root. Toolchain pinned in `rust-toolchain.toml` (stable + clippy +
rustfmt).

## What exists

Workspace: two crates under `crates/`.

### `clinker-m68k` — the CPU (no machine knowledge)

| File | State |
|---|---|
| `src/bus.rs` | `Bus` trait. Clock-driven: `advance(cycles)` + `interrupt_level()` (both no-op default), plus `read/write_u8/16/32` (big-endian, odd-address → `BusError::Address`). `Width::bus_cycles()` = 4/4/8. |
| `src/registers.rs` | `Registers` (D0-7, A0-6, USP/SSP with S-bit banking via `addr()`/`sp()`), `StatusRegister` bit helpers, `ConditionCode`. |
| `src/decode.rs` | **STUB table.** Recognises `NOP RESET RTS TRAP#n MOVEQ BRA.s ILLEGAL` + line-A/F. Everything else → `Decoded::Unimplemented`. Has unit tests. |
| `src/execute.rs` | Executes the stub set. Arms return *internal* cycles only (bus cycles charged by the ticked helpers). |
| `src/exception.rs` | `Exception` enum → vector numbers. `vector_offset()`. |
| `src/cpu.rs` | `Cpu`, `reset()`, `step()` (interrupt sample → fetch → decode → execute → vector), `enter_exception()` (pushes 68000 **6-byte** frame only), ticked `mem_read/write_*` helpers shared with `execute`. |
| `src/lib.rs` | Re-exports; `CLOCK_HZ`, `ADDRESS_MASK`. |

### `clinker-core` — the machine (owns address decode)

| File | State |
|---|---|
| `src/system_bus.rs` | `SystemBus` implements spec §3.1: 16 MB RAM populated, ROM at `$FC0000` (128 KB), inert display (`$FE0000`) + I/O (`$FF0000`) windows (read `$FF`, writes dropped), reset overlay (ROM low 1 KB aliased at `$0`, cleared by writing 0 to bit 0 of `$FF0000`). Unit tests. |
| `src/machine.rs` | `Machine { cpu, bus }` + `load_rom` / `load_ram` / `reset` / `step` / `run`. Integration tests build a boot ROM and drive it. |
| `src/lib.rs` | Front-end boundary notes; re-exports. |

## Decisions locked (do not relitigate)

Recorded in `docs/CLINKER-68000-SPEC.md` §3.1 / §10.1 / §10.2 and
`docs/ARCHITECTURE.md`:

1. **Timing:** clock-driven core, accurate per-instruction cycle *totals*, no
   prefetch-queue modelling. Full cycle-exact stays a possible additive upgrade,
   not planned.
2. **Address map:** full 16 MB RAM, I/O+ROM+display in the top 256 KB, reset ROM
   overlay at `$0`. Table in spec §3.1.
3. **Exceptions:** MC68000 (not 010/020) → 6-byte frame for groups 1 & 2.
   Group-0 (bus/address error) will get a *diagnostic-grade* 14-byte frame when
   built; no MMU → bus error is fatal, instruction restart is not a goal.
4. **License:** `GPL-3.0-or-later` (Cargo tag + README notice agree).
5. **Crate split:** CPU crate talks only to `Bus`; machine crate owns decode. A
   third FFI crate comes later. GTK front end is a separate repo.

## Next steps (priority order)

1. **Real instruction decoder.** Replace the `decode.rs` stub with the full
   16-bit opcode map (generated dispatch table is the standard approach —
   Musashi/Moira both do this). Build out `execute.rs` with proper
   effective-address handling. Validate against **TomHarte/ProcessorTests
   `680x0`** JSON vectors (final state first; per-cycle later).
2. **Group-0 stack frame.** `cpu.rs::enter_exception` currently pushes the
   6-byte frame for everything; add the 14-byte bus/address-error frame
   (diagnostic-grade fields — fault addr + R/W + FC accurate, murky bits 0).
3. **`STOP` / `TRACE`** — `RunState::Stopped` exists but is unreachable; T-bit is
   read on reset but never acted on.
4. **Exception priority / simultaneity** — lands with interrupt sources, not
   before.

## Blocked on user input (see spec "Open Questions")

- **Boot ROM size / contents** — map provisionally reserves 128 KB at `$FC0000`.
  Depends on whether the primary language/OS is resident in ROM or loaded from
  floppy. Doesn't block CPU work; does block finalising the map.
- **I/O register layout** — the `$FF0000` page is allocated as a block; per-chip
  offsets (FDC, 5380, DUART, Centronics, ACIA ×2, OPL2, keyboard, system-control)
  unassigned. Blocks the first peripheral.
- **System-control register** — address + bit position for the overlay-clear bit
  (currently assumed `$FF0000` bit 0).

## Out of scope for this repo

`clinker-6502` is now its **own separate repo** (`katmore/clinker-6502`), handled
in a different session. This repo stays 68000-only.

## Gotchas

- `SystemBus::Target::Unmapped` is currently unreachable (the map covers the
  whole 24-bit space). Kept as a defensive branch.
- Reset overlay is active after `reset()`. Test/boot code that expects RAM at
  low addresses must clear it first (write 0 to `$FF0000`) or live with ROM
  shadowing the low 1 KB. `Machine` test helper builds a boot ROM instead.
- `cargo fmt` will reflow files; re-`Read` before `Edit` if you ran it.
