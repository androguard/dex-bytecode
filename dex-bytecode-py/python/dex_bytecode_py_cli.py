"""
CLI to disassemble Dalvik bytecode (raw instruction bytes).
Input: file path, stdin, or hex-encoded bytes (--hex).
"""

import argparse
import re
import sys


def parse_hex(hex_str: str) -> bytes:
    """Parse hex string (with or without spaces) into bytes."""
    s = re.sub(r"\s+", "", hex_str)
    if len(s) % 2 != 0:
        raise ValueError("hex string must have an even number of nibbles")
    return bytes.fromhex(s)


def main() -> None:
    parser = argparse.ArgumentParser(
        description="Disassemble Dalvik bytecode (raw instruction bytes).",
        epilog="Input: file path, stdin (-), or --hex.",
    )
    parser.add_argument(
        "-i",
        "--input",
        metavar="FILE",
        default=None,
        help="Input file path, or '-' for stdin (default: stdin if no --hex)",
    )
    parser.add_argument(
        "--hex",
        metavar="STR",
        default=None,
        help='Hex-encoded bytecode (e.g. "700001000000" or "70 00 01 00"). Conflicts with -i.',
    )
    parser.add_argument(
        "-o",
        "--offset",
        type=lambda x: int(x, 0),
        default=0,
        metavar="N",
        help="Start offset in bytes (default: 0)",
    )
    parser.add_argument(
        "-l",
        "--labels",
        action="store_true",
        help="Show labels at branch targets (e.g. :L00000010)",
    )
    parser.add_argument(
        "--version",
        action="version",
        version="%(prog)s " + _get_version(),
    )
    args = parser.parse_args()

    if args.hex is not None and args.input is not None:
        parser.error("--hex and --input are mutually exclusive")

    try:
        import dex_bytecode_py
    except ImportError as e:
        print("dex-dis: failed to import dex_bytecode_py:", e, file=sys.stderr)
        print("Install the package with: maturin develop -m dex-bytecode-py/Cargo.toml", file=sys.stderr)
        sys.exit(1)

    disassemble_fn = dex_bytecode_py.disassemble
    get_branch_targets_fn = getattr(dex_bytecode_py, "get_branch_targets", None)

    if args.hex is not None:
        try:
            data = parse_hex(args.hex)
        except ValueError as e:
            print(f"dex-dis: invalid hex: {e}", file=sys.stderr)
            sys.exit(1)
    elif args.input == "-" or args.input is None:
        data = sys.stdin.buffer.read()
    else:
        try:
            with open(args.input, "rb") as f:
                data = f.read()
        except OSError as e:
            print(f"dex-dis: {e}", file=sys.stderr)
            sys.exit(1)

    if not data:
        print("dex-dis: no input data to decode.", file=sys.stderr)
        sys.exit(1)

    if args.offset >= len(data):
        print(
            f"dex-dis: offset {args.offset} past end of data ({len(data)} bytes).",
            file=sys.stderr,
        )
        sys.exit(1)

    try:
        instructions = disassemble_fn(data, args.offset)
    except ValueError as e:
        print(f"dex-dis: disassembly error: {e}", file=sys.stderr)
        sys.exit(1)

    label_offsets_rel: set[int] = set()
    if args.labels and get_branch_targets_fn is not None:
        try:
            label_offsets_rel = set(get_branch_targets_fn(data, args.offset))
        except ValueError:
            pass

    for ins in instructions:
        abs_offset = ins["offset"]
        rel_offset = abs_offset - args.offset
        if args.labels and rel_offset in label_offsets_rel:
            print(f":L{abs_offset:08x}")
        opcode = ins["opcode"]
        mnemonic = ins["mnemonic"]
        operands = ins["operands"]
        print(f"{abs_offset:08x}  {opcode:02x}{'':18} {mnemonic} {operands}")

    print(f"\n; {len(instructions)} instruction(s)")


def _get_version() -> str:
    try:
        import dex_bytecode_py
        return getattr(dex_bytecode_py, "__version__", "0.0.1")
    except ImportError:
        return "0.0.1"


if __name__ == "__main__":
    main()
