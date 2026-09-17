# dex-bytecode

DEX bytecode disassembler and assembler (Rust core) with Python bindings.

<p align="center"><img width="120" src="./.github/logo.png"></p>
<h2 align="center">DEX-BYTECODE</h2>

<div align="center">

![Powered By: Androguard](https://img.shields.io/badge/androguard-green?style=for-the-badge&label=Powered%20by&link=https%3A%2F%2Fgithub.com%2Fandroguard)

</div>

**Contents:** [Features](#features) · [Documentation](#documentation) · [Usage](#usage) · [Development](#development)

## Features

- **Dalvik instruction decoder** compatible with [androguard’s dex module](https://github.com/androguard/androguard/blob/master/androguard/core/dex/__init__.py)
  - Linear-sweep decoding
  - Standard Dalvik formats covered by the opcode table (10x, 12x, 21c, 22c, 31t, 35c, 3rc, …)
  - Payload pseudo-instructions: packed/sparse-switch, fill-array-data
- **Formatted operands** like androguard (e.g. `v0, v1`, `v0, string@5`)
- **Pluggable reference resolution** (`ResolveRef`, `FnResolver`) for string/type/field/method/proto/callsite display names
- **Control-flow helpers**
  - `branch_targets(data, offset)` — byte offset(s) the instruction may jump to
  - `explicit_successors(data, offset)` — CFG successors (switch expanded; fill-array-data yields none)
  - `is_unconditional_branch(data, offset)` — true for goto / goto/16 / goto/32 (no fallthrough)
  - `collect_branch_targets(instructions, data, base_offset)`
  - `basic_blocks(instructions, data, base_offset)` — blocks with `successors` (deduplicated, sorted) and `fallthrough_to`
  - `cfg_edges(instructions, data, base_offset)` — full CFG edges including fallthrough
  - `exception_edges(entries, blocks)` — exception edges (block start → handler) for try/catch
- **Try/catch formatting**: `TryCatchEntry`, `format_catch_line`
- **Patching / minimal encoding**: `patch_branch_target(data, from_offset, to_offset)`, `encode_nop()`, `encode_return_void()`, `encode_goto(rel_units)`
- **CLI disassembler** (Rust): file/stdin/hex input, labels, basic-block view (`--blocks`), DOT export (`--dot`), `--no-addresses`, `--color always|never|auto`, `--quiet`. Optimized for large files (buffered output, one-pass branch-target precompute).
- **Python bindings + CLI**: `dex-bytecode-py` provides `disassemble()`, `decode_instruction()`, `basic_blocks()`, `cfg_edges()`, `patch_branch()`, encode helpers, and a `dex-dis` command

## Documentation

### Overview

```
  raw bytecode          decode_all()           basic_blocks()        cfg_edges()
  (bytes)               linear sweep           block boundaries      (from, to)
       │                      │                       │                    │
       ▼                      ▼                       ▼                    ▼
  ┌─────────┐           ┌───────────┐           ┌───────────┐        ┌───────────┐
  │ 28 02   │           │ Instruction│           │ Block 0   │        │ (0, 4)    │
  │ 00 00   │  ──────►  │ Instruction│  ──────►  │ Block 1   │  ───►  │ (2, 4)    │  (DOT, etc.)
  │ ...     │           │ ...       │           │ Block 2   │        │ ...       │
  └─────────┘           └───────────┘           └───────────┘        └───────────┘
```

### Decoding (linear sweep)

Instructions are decoded in order from the byte buffer. Each instruction’s length is determined by its format (2, 4, 6, … bytes); the next instruction starts at `offset + length`.

```
  offset 0    2    4    6    8    ...
         ├────┬────┬────┬────┬────
         │ins0│ins1│ins2│ins3│ ...
         │ 2b │ 2b │ 2b │ 2b │
         └────┴────┴────┴────┴────
         decode_one() at 0 → ins0, len 2
         decode_one() at 2 → ins1, len 2
         decode_all() → [ins0, ins1, ins2, ...]
```

### Control flow

- **Branch targets** — `branch_targets(data, offset)` returns the byte offset(s) the instruction at `offset` may jump to (one for goto/if-*, or the payload offset for switch). Use for labels. Dalvik branch offsets are relative to the **current** instruction address (`target = offset + signed_units * 2`).
- **Explicit successors** — `explicit_successors(data, offset)` is CFG-aware: for packed/sparse-switch it returns all case targets; for fill-array-data it returns none. Use with `basic_blocks`.
- **Unconditional branches** — `is_unconditional_branch(data, offset)` returns true for goto (0x28), goto/16 (0x29), goto/32 (0x2a). Used internally to set `fallthrough_to` (none for these).
- **Basic blocks** — `basic_blocks(instructions, data, base_offset)` splits code into contiguous blocks. Each block has `start_offset`, `end_offset`, `successors` (branch targets, deduplicated and sorted), and `fallthrough_to` (next block when the block does not end with an unconditional goto).

  Example: `goto +2` at 0 → target 4. Block boundaries at 0 (entry), 2 (after goto), 4 (target).

  ```
  offset   0    2    4    6    8
           ├────┤    ├────┤    ├──── ...
           │goto│    │nop │    │nop
           │ +2 │    │    │    │nop
           └──┬─┘    └──┬─┘    │ret
              │         │      └────
   block 0 ───┘         │
   successors=[4]       │  fallthrough_to = 4
   no fallthrough       │
   (ends with goto)     │  block 1
                        └──► block 2 start (:L00000004)
  ```

- **CFG edges** — `cfg_edges(instructions, data, base_offset)` returns all edges `(from, to)` including fallthrough. Use for graph algorithms or DOT export.

  ```
       block 0              block 1              block 2
   ┌─────────────┐      ┌─────────────┐      ┌─────────────┐
   │ goto → 4    │      │ nop         │      │ nop         │
   │             │      │             │      │ nop         │
   └──────┬──────┘      └──────┬──────┘      │ return-void │
          │  branch            │  fallthrough └─────────────┘
          │                    │
          └────────────────────┴──────────────► block 2
  ```

- **Exception edges** — `exception_edges(try_catch_entries, blocks)` returns edges `(block_start, handler_offset)` for every block whose range overlaps a try range. Combine with `cfg_edges` for a full CFG including exception flow.

  ```
   try range [0..8)              try range [8..20)
   ┌────────────────────────┐   ┌────────────────────────┐
   │  block 0   block 1    │   │  block 2   block 3     │
   │  [0..4)    [4..8)     │   │  [8..16)   [16..24)    │
   └────────┬───────────────┘   └────────┬───────────────┘
            │  exception                  │  exception
            ▼                             ▼
        handler @ 16                  handler @ 24
  ```

### Patching and encoding

- **`patch_branch_target(data, from_offset, to_offset)`** — Rewrites the branch at `from_offset` so it jumps to `to_offset` (byte offset). Supports F10t (goto), F20t (goto/16), F21t (if-*z), F22t (if-*), F30t (goto/32). Returns `Err` if the instruction is not a branch or the relative offset is out of range.
- **`encode_nop()`**, **`encode_return_void()`** — Return the 2-byte encoding of nop and return-void.
- **`encode_goto(rel_units)`** — Returns the 2-byte F10t encoding of goto with the given signed 8-bit offset in 16-bit code units.

### Performance (large files)

- **Decoder**: `decode_all` reserves capacity (`remaining / 2`) to reduce reallocations.
- **CLI**: Branch targets are precomputed in one pass (`targets_per_ins`, `label_offsets`) so the main loop does not call `branch_targets` per instruction. Output is written through a `BufWriter` to reduce syscalls.

### CLI options

| Option | Description |
|--------|-------------|
| `-i`, `--input` | Input file or `-` for stdin (default when no `--hex`) |
| `--hex` | Hex-encoded bytecode (e.g. `28020000000000000e00`) |
| `-o`, `--offset` | Start offset in bytes (default: 0) |
| `-l`, `--labels` | Show labels at branch targets (`:L00000010`) |
| `-b`, `--blocks` | Show basic-block structure with arrows (implies `--labels`) |
| `--dot` | Output CFG in DOT (Graphviz) format instead of disassembly |
| `--color` | `always`, `never`, or `auto` (default: use color when stdout is a TTY) |
| `--no-addresses` | Omit the address column in disassembly |
| `-q`, `--quiet` | Omit trailing comment (instruction/block count) |
| `-d`, `--debug` | Enable debug logs |

## Usage

### Rust library

**Decode and iterate** (bytecode: `nop`; `return-void`):

```rust
use dex_bytecode::{decode_all, decode_one, Decoder};

let data = [0x00u8, 0x00, 0x0e, 0x00]; // nop; return-void

let ins0 = decode_one(&data, 0).unwrap();
assert_eq!(ins0.mnemonic(), "nop");
assert_eq!(ins0.length(), 2);

let all = decode_all(&data, 0).unwrap();
assert_eq!(all.len(), 2);
assert_eq!(all[1].mnemonic(), "return-void");

let mut it = Decoder::new(&data, 0, None);
while let Some(Ok(ins)) = it.next() {
    println!("{:08x} {} {}", ins.offset, ins.mnemonic(), ins.operands());
}
```

Output:

```
00000000 nop
00000002 return-void
```

**Control flow** (bytecode: `goto +2`; `nop`; `nop`; `nop`; `return-void` — hex `28020000000000000e00`):

```rust
use dex_bytecode::{
    basic_blocks, branch_targets, cfg_edges, collect_branch_targets, decode_all,
};

// goto +02h (target = 4); nop; nop; nop; return-void
let data: &[u8] = &[
    0x28, 0x02, // goto +2  → offset 4
    0x00, 0x00, // nop
    0x00, 0x00, // nop   ← label :L00000004
    0x00, 0x00, // nop
    0x0e, 0x00, // return-void
];

let instructions = decode_all(data, 0).unwrap();
for ins in &instructions {
    println!("{:08x}  {} {}", ins.offset, ins.mnemonic(), ins.operands());
}

let labels = collect_branch_targets(&instructions, data, 0);
assert!(labels.contains(&4));

assert_eq!(branch_targets(data, 0), vec![4u32]);

let blocks = basic_blocks(&instructions, data, 0);
assert_eq!(blocks.len(), 3);
assert_eq!(blocks[0].successors, vec![4u32]);      // goto
assert!(blocks[0].fallthrough_to.is_none());        // unconditional
assert_eq!(blocks[1].fallthrough_to, Some(4));      // fallthrough into target

let edges = cfg_edges(&instructions, data, 0);
assert!(edges.contains(&(0, 4))); // branch
assert!(edges.contains(&(2, 4))); // fallthrough
```

Output:

```
00000000  goto +02h
00000002  nop
00000004  nop
00000006  nop
00000008  return-void
```

**Patching:**

```rust
use dex_bytecode::{decode_all, encode_goto, encode_nop, encode_return_void, patch_branch_target};

let mut data = [
    0x28, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0e, 0x00,
];
assert_eq!(&encode_goto(2)[..], &data[0..2]);
assert_eq!(&encode_nop()[..], &[0x00, 0x00]);
assert_eq!(&encode_return_void()[..], &[0x0e, 0x00]);

// Retarget the goto from offset 4 to offset 6
patch_branch_target(&mut data, 0, 6).unwrap();
let instructions = decode_all(&data, 0).unwrap();
assert_eq!(instructions[0].operands(), "+03h"); // +3 units → byte offset 6
```

**Try/catch (exception edges):**

```rust
use dex_bytecode::{basic_blocks, decode_all, exception_edges, format_catch_line, TryCatchEntry};

let data: &[u8] = &[0x00, 0x00, 0x00, 0x00, 0x0e, 0x00]; // nop; nop; return-void
let instructions = decode_all(data, 0).unwrap();
let blocks = basic_blocks(&instructions, data, 0);

let entry = TryCatchEntry {
    start_offset: 0,
    end_offset: 4,
    handler_offset: 4,
    type_index: Some(1),
};
println!("{}", format_catch_line(&entry, Some("Ljava/lang/Exception;")));
// .catch Ljava/lang/Exception; { 0x00000000 .. 0x00000004 } :L00000004

let ex = exception_edges(&[entry], &blocks);
// edges from every block overlapping [0,4) → handler 4
```

### CLI (Rust)

From the repo root:

```bash
# Disassemble from hex
cargo run --bin dex-bytecode-dis -- --hex "28020000000000000e00" -q
```

```
00000000  28                   goto +02h
00000002  00                   nop
00000004  00                   nop
00000006  00                   nop
00000008  0e                   return-void
```

```bash
# Labels + basic blocks
cargo run --bin dex-bytecode-dis -- --hex "28020000000000000e00" -b -q --color never
```

```
╭── block 0 → :L00000004
│ 00000000  28                   goto +02h
│   → :L00000004
╭── block 1   (no successors)
│ 00000002  00                   nop
╭── block 2   (no successors)
:L00000004
│ 00000004  00                   nop
│ 00000006  00                   nop
│ 00000008  0e                   return-void
╰──
```

```bash
# DOT (Graphviz) CFG
cargo run --bin dex-bytecode-dis -- --hex "28020000000000000e00" --dot
```

```
digraph cfg {
  rankdir=TB;
  node [shape=box, fontname="monospace"];
  b0 [label="block 0\n0x00000000..0x00000002"];
  b1 [label="block 1\n0x00000002..0x00000004"];
  b2 [label="block 2\n0x00000004..end"];
  b0 -> b2;
  b1 -> b2;
}
```

Other useful flags:

```bash
cargo run --bin dex-bytecode-dis -- -i path/to/bytecode.bin
cat path/to/bytecode.bin | cargo run --bin dex-bytecode-dis -- -i -
cargo run --bin dex-bytecode-dis -- -i file.bin -o 16
cargo run --bin dex-bytecode-dis -- -i file.bin -l
cargo run --bin dex-bytecode-dis -- -i file.bin --dot > cfg.dot   # then: dot -Tpng cfg.dot -o cfg.png
cargo run --bin dex-bytecode-dis -- -i file.bin --no-addresses
cargo run --bin dex-bytecode-dis -- -i file.bin --color always
cargo run --bin dex-bytecode-dis -- -i file.bin -q
```

Notes:
- The workspace sets `default-members = ["dex-bytecode"]`, so running the disassembler does **not** build `dex-bytecode-py` unless you explicitly build it.
- `-b/--blocks` implies labels.

### Python bindings + Python CLI

See [`dex-bytecode-py/README.md`](dex-bytecode-py/README.md) for full API details.

**Install** (from repo root; use the same virtualenv you want to test with):

```bash
# Activate your venv first, then:
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m dex-bytecode-py/Cargo.toml
```

**Disassemble and CFG** (same bytecode as the Rust example):

```python
from dex_bytecode_py import (
    disassemble,
    decode_instruction,
    get_branch_targets,
    basic_blocks,
    cfg_edges,
    patch_branch,
    encode_goto_bytes,
    encode_nop_bytes,
    encode_return_void_bytes,
)

# goto +2; nop; nop; nop; return-void
bc = bytes.fromhex("28020000000000000e00")

for ins in disassemble(bc):
    print(f"{ins['offset']:08x}  {ins['mnemonic']:12} {ins['operands']}")
# 00000000  goto         +02h
# 00000002  nop
# 00000004  nop
# 00000006  nop
# 00000008  return-void

print(decode_instruction(bc, 0)["disasm"])   # "goto +02h"
print(sorted(get_branch_targets(bc)))        # [4]

for i, b in enumerate(basic_blocks(bc)):
    print(
        f"block {i}: [{b['start_offset']},{b['end_offset']}) "
        f"succ={b['successors']} ft={b['fallthrough_to']}"
    )
# block 0: [0,2) succ=[4] ft=None
# block 1: [2,4) succ=[] ft=4
# block 2: [4,10) succ=[] ft=None

print([(e["from"], e["to"]) for e in cfg_edges(bc)])
# [(0, 4), (2, 4)]

# Patch goto target from 4 → 6, then re-disassemble
patched = patch_branch(bc, 0, 6)
print(disassemble(patched)[0]["operands"])  # "+03h"

assert encode_goto_bytes(2) == bytes([0x28, 0x02])
assert encode_nop_bytes() == bytes([0x00, 0x00])
assert encode_return_void_bytes() == bytes([0x0e, 0x00])
```

**Quick one-liner** (must use the same Python that maturin installed into):

```bash
python -c "from dex_bytecode_py import disassemble; print(disassemble(bytes.fromhex('28020000000000000e00')))"
```

If you get `ModuleNotFoundError: No module named 'dex_bytecode_py'`, activate the venv first (e.g. `source .venv/bin/activate`) and try again.

**Python CLI** (`dex-dis`):

```bash
dex-dis --hex "28020000000000000e00" --labels
dex-dis --hex "28020000000000000e00"
dex-dis -i path/to/bytecode.bin
```

**Run Python tests:**

```bash
cd dex-bytecode-py && python -m unittest discover -s tests -v
```

## Development

### Tests

```bash
cargo test -p dex-bytecode
```

Regression tests include:

- **Control flow**: basic blocks, CFG edges, fallthrough, exception edges, packed/sparse switch, fill-array-data, `is_unconditional_branch`, successor deduplication.
- **Patching**: `patch_branch_target`, `encode_nop` / `encode_return_void` / `encode_goto`.
- **Disassembler**: one-pass precompute (label set and per-instruction targets) matches `collect_branch_targets` and per-call `branch_targets`; `decode_all` on large bytecode is idempotent.

### Examples

```bash
cargo run -p dex-bytecode --example resolve_example
cargo run -p dex-bytecode --example control_flow_example
cargo run -p dex-bytecode --example try_catch_example
```

## License

Distributed under the [Apache License, Version 2.0](LICENSE).