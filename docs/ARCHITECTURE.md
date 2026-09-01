# Clinker 68000 — Emulator Architecture

Companion to [`CLINKER-68000-SPEC.md`](CLINKER-68000-SPEC.md), which is the source
of truth for the machine. This file covers how the *emulator* is structured and
what is / isn't built yet.

## Workspace layout

```
clinker-68000/
├── Cargo.toml                  workspace root
├── rust-toolchain.toml         pins stable + clippy + rustfmt
├── crates/
│   ├── clinker-m68k/           the CPU — no I/O, no machine knowledge
│   │   └── src/
│   │       ├── lib.rs          public surface + constants (CLOCK_HZ, ADDRESS_MASK)
│   │       ├── bus.rs          `Bus` trait — the CPU's only route to memory/I/O
│   │       ├── registers.rs    D0-7, A0-6, USP/SSP, PC, SR (with A7 banking)
│   │       ├── decode.rs       opcode -> `Decoded` (STUB table, see below)
│   │       ├── execute.rs      per-instruction execution
│   │       ├── exception.rs    vector numbers / `Exception` enum
│   │       └── cpu.rs          `Cpu`, `reset()`, clock-driven `step()`, vectoring
│   └── clinker-core/           the machine — owns the address map
│       └── src/
│           ├── lib.rs          front-end boundary notes
│           ├── system_bus.rs   `SystemBus` — address decoder (spec §3.1) + reset overlay
│           └── machine.rs      `Machine { cpu, bus }` + reset/step/run + tests
└── docs/
```

### Why two crates

`clinker-m68k` is a self-contained 68000 that talks to a `Bus` trait and nothing
else — it never learns that it's inside a Clinker. `clinker-core` supplies the
`Bus` implementation (`SystemBus`), owns address decode, and will grow the
peripherals. Same split as Musashi (CPU) / an embedding project (machine); keeps
the CPU independently testable.

A third crate — a C ABI / FFI shim for the GTK front end — will be added when
there's something for the front end to talk to. The GTK binary lives in a
separate repo (spec §10.4).

## What runs today

- **Register file** with A7/USP/SSP banking, SR helpers.
- **Clock-driven `Bus`**: `advance(cycles)` is called through the instruction,
  not just at the end; `interrupt_level()` is sampled each instruction boundary.
  Prefetch queue *not* modelled yet (see below) — `advance` granularity is
  bus-transaction + internal-cycle-group, which is the right shape to tighten.
- **`SystemBus` address decoder** implementing spec §3.1: 15.75 MB RAM, 128 KB
  ROM at `$FC0000`, inert 64 KB display + 64 KB I/O windows, and the reset
  overlay (ROM's low 1 KB aliased at `$0`, dropped by writing 0 to bit 0 of the
  system-control register at `$FF0000`).
- **`step()` loop**: interrupt sample → fetch → `decode` → `execute` → vectoring.
- **Exception vectoring**: 68000 6-byte frame (SR, PC); double fault → `Halted`.
- Decoder recognises `NOP`, `RESET`, `RTS`, `TRAP #n`, `MOVEQ`, `BRA.s`,
  `ILLEGAL`, line-A / line-F. Everything else → Illegal Instruction. **This is a
  stub table**, enough to prove the loop; not a working instruction set.

## Not built this pass (deliberately)

Rest of the instruction set + effective-address decode; peripherals behind the
display/I/O windows; interrupt *sources*; the FFI crate; the GTK front end.

## Reference implementations studied

No third-party code is vendored or copied. Read for behaviour/structure:

| Reference | Use |
|---|---|
| **Motorola M68000 Family Programmer's Reference Manual** (M68000PM/AD) | Instruction semantics, addressing modes, exception model, SR layout. |
| **MC68000 User's Manual** (MC68000UM/AD) | Bus cycles, reset timing, exception stack frames, signal-level behaviour. |
| **Musashi** (Karl Stenerud, C) | Core structure: generated opcode jump table, EA handler dispatch, cycle-count tables. Studying the approach only. |
| **Moira** (Dirk W. Hoffmann, C++, vAmiga) | Well-tested, optionally cycle-exact 68000. Reference for prefetch modelling and our own test expectations. |
| **yacht.txt** ("Yet Another Cycle Hunting Table") | Per-instruction cycle counts. |
| **TomHarte/ProcessorTests** `680x0` JSON vectors | Per-instruction conformance (incl. per-cycle bus transactions) as the decoder fills in. |
| **r68k / m68000-emu (Rust)** | Prior art; API-shape sanity check. |

## Decisions locked (see spec §3.1, §10.1, §10.2)

- **Timing model:** clock-driven core, accurate per-instruction cycle totals, no
  prefetch-queue modelling. Full cycle-exact stays a possible *additive* upgrade.
- **Address map:** full 16 MB RAM populated, I/O + ROM + display in the top
  256 KB, reset ROM overlay at `$0`.
- **Exception model:** MC68000 (not 010/020) → 6-byte frame for groups 1 & 2.
  Group-0 (bus/address error) gets a diagnostic-grade 14-byte frame *when built*;
  no MMU → bus error is a fatal path, instruction restart is not a goal.

## Still open / not yet done

1. **Group-0 stack frame.** `enter_exception` currently pushes the 6-byte frame
   for *everything*, including bus/address error. The 14-byte frame is needed
   before running code that inspects it (i.e. once there's a boot ROM).
2. **Prefetch queue (IRC/IR/IRD).** Not modelled. Only needed if a real title
   depends on exact bus timing; upgrade is additive on the clock-driven design.
3. **Not yet decoded:** `STOP` (`RunState::Stopped` exists but is unreachable),
   `TRACE` (T-bit read on reset, never acted on), privileged instructions beyond
   `RESET`, and exception priority / simultaneity (lands with interrupt sources).
4. **ROM size / contents** and the **per-chip I/O register layout** — tracked in
   the spec's Open Questions; both block the first peripheral, neither blocks
   more CPU work.
