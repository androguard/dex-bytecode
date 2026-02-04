# dex-bytecode-py

Python bindings for the **dex-bytecode** Rust library: a Dalvik instruction decoder compatible with [androguard](https://github.com/androguard/androguard) style decoding (see `androguard/core/dex/__init__.py`).

## Installation

Build and install with [maturin](https://github.com/PyO3/maturin) (requires Rust and Python):

```bash
cd dex-bytecode-py
pip install maturin
maturin develop
```

Or from the workspace root:

```bash
pip install maturin
maturin develop -m dex-bytecode-py/Cargo.toml
```

For Python 3.14+, you may need:

```bash
PYO3_USE_ABI3_FORWARD_COMPATIBILITY=1 maturin develop -m dex-bytecode-py/Cargo.toml
```

**Tip:** maturin installs into the **active** virtualenv (or the one it detects). Use that same Python to import the module. If you get `ModuleNotFoundError: No module named 'dex_bytecode_py'`, activate the venv first (e.g. `source .venv/bin/activate`) then run `python` or `dex-dis`.

## Usage

```python
from dex_bytecode_py import disassemble, decode_instruction

# Disassemble raw Dalvik bytecode (e.g. from a DEX code_item insns)
bytecode = bytes([0x00, 0x00, 0x01, 0x21])  # nop, move v1, v2
instructions = disassemble(bytecode)
for ins in instructions:
    print(ins["offset"], ins["mnemonic"], ins["operands"])
# 0 nop
# 2 move v1, v2

# Single instruction at offset
ins = decode_instruction(bytecode, 2)
print(ins["disasm"])  # "move v1, v2"
```

Each instruction is a dict with:

- **offset** (int): byte offset in the buffer
- **length** (int): instruction size in bytes
- **opcode** (int): raw opcode (0x00–0xFF)
- **mnemonic** (str): e.g. `"move"`, `"invoke-virtual"`
- **operands** (str): e.g. `"v0, v1"`, `"v0, string@5"`
- **disasm** (str): full line `mnemonic operands`

Reference operands (strings, types, fields, methods) are shown as `kind@index` when the DEX constant pool is not available (e.g. `string@3`, `method@10`).

You can also use `get_branch_targets(data, offset)` to get the set of byte offsets where branch targets land (for displaying labels in a disassembler).

## CLI

After installing, a `dex-dis` command is available to disassemble bytecode from a file, stdin, or hex:

```bash
# From stdin (pipe or type)
echo -n $'\x00\x00\x01\x21' | dex-dis

# From a file
dex-dis -i path/to/bytecode.bin

# From hex (with or without spaces)
dex-dis --hex "700001000000"
dex-dis --hex "70 00 01 00 00 00"

# Start at an offset
dex-dis -i file.bin -o 16

# Show labels at branch targets
dex-dis -i file.bin --labels
```

Options:

- **-i, --input** *FILE*: Input file path, or `-` for stdin (default: stdin if no `--hex`).
- **--hex** *STR*: Hex-encoded bytecode; conflicts with `-i`.
- **-o, --offset** *N*: Start offset in bytes (default: 0).
- **-l, --labels**: Print `:L` labels at branch target offsets.

You can also run the CLI as a module: `python -m dex_bytecode_py_cli` (with the same options).

## Tests

After building the extension (`maturin develop -m dex-bytecode-py/Cargo.toml` from repo root, or `maturin develop` from `dex-bytecode-py`), run the Python tests:

```bash
cd dex-bytecode-py
python -m unittest discover -s tests -v
```

Or run the test file directly:

```bash
python -m unittest tests.test_dex_bytecode_py -v
```
