"""Tests for dex_bytecode_py Python bindings."""

import unittest

# Import after building: maturin develop -m dex-bytecode-py/Cargo.toml
try:
    from dex_bytecode_py import disassemble, decode_instruction, __version__
except ImportError as e:
    raise ImportError(
        "Build the extension first: maturin develop -m dex-bytecode-py/Cargo.toml"
    ) from e


class TestModule(unittest.TestCase):
    def test_version(self):
        self.assertIsInstance(__version__, str)
        self.assertTrue(len(__version__) >= 1)


class TestDecodeInstruction(unittest.TestCase):
    def test_nop(self):
        bytecode = bytes([0x00, 0x00])
        ins = decode_instruction(bytecode)
        self.assertIsInstance(ins, dict)
        self.assertEqual(ins["offset"], 0)
        self.assertEqual(ins["length"], 2)
        self.assertEqual(ins["opcode"], 0x00)
        self.assertEqual(ins["mnemonic"], "nop")
        self.assertEqual(ins["operands"], "")
        self.assertEqual(ins["disasm"], "nop")

    def test_nop_at_offset(self):
        bytecode = bytes([0x00, 0x00, 0x00, 0x00])
        ins = decode_instruction(bytecode, offset=2)
        self.assertEqual(ins["offset"], 2)
        self.assertEqual(ins["mnemonic"], "nop")

    def test_move(self):
        bytecode = bytes([0x01, 0x21])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "move")
        self.assertEqual(ins["operands"], "v1, v2")
        self.assertEqual(ins["length"], 2)
        self.assertIn("move", ins["disasm"])

    def test_return_void(self):
        bytecode = bytes([0x0E, 0x00])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "return-void")
        self.assertEqual(ins["operands"], "")

    def test_const_string(self):
        # const-string v0, string@5 (21c: AA=0, BBBB=5)
        bytecode = bytes([0x1A, 0x00, 0x05, 0x00])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "const-string")
        self.assertEqual(ins["operands"], "v0, string@5")
        self.assertEqual(ins["length"], 4)

    def test_dict_keys(self):
        bytecode = bytes([0x00, 0x00])
        ins = decode_instruction(bytecode)
        for key in ("offset", "length", "opcode", "mnemonic", "operands", "disasm"):
            self.assertIn(key, ins, f"missing key {key}")

    def test_offset_past_end(self):
        bytecode = bytes([0x00, 0x00])
        with self.assertRaises(ValueError):
            decode_instruction(bytecode, offset=10)

    def test_empty_data(self):
        bytecode = bytes([])
        with self.assertRaises(ValueError):
            decode_instruction(bytecode)

    def test_invalid_bytecode(self):
        # 0xFF 0xAB is start of const-method-type which needs 6 bytes
        bytecode = bytes([0xFF, 0xAB])
        with self.assertRaises(ValueError):
            decode_instruction(bytecode)


class TestDisassemble(unittest.TestCase):
    def test_empty_sequence(self):
        # Single nop then we only have 2 bytes
        bytecode = bytes([0x00, 0x00])
        instructions = disassemble(bytecode)
        self.assertEqual(len(instructions), 1)
        self.assertEqual(instructions[0]["mnemonic"], "nop")

    def test_nop_nop_return_void(self):
        bytecode = bytes([0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x0E, 0x00])
        instructions = disassemble(bytecode)
        self.assertEqual(len(instructions), 4)
        self.assertEqual(instructions[0]["mnemonic"], "nop")
        self.assertEqual(instructions[1]["mnemonic"], "nop")
        self.assertEqual(instructions[2]["mnemonic"], "nop")
        self.assertEqual(instructions[3]["mnemonic"], "return-void")
        total_len = sum(ins["length"] for ins in instructions)
        self.assertEqual(total_len, len(bytecode))

    def test_offsets_sequential(self):
        bytecode = bytes([0x00, 0x00, 0x01, 0x21, 0x0E, 0x00])
        instructions = disassemble(bytecode)
        self.assertEqual(instructions[0]["offset"], 0)
        self.assertEqual(instructions[1]["offset"], 2)
        self.assertEqual(instructions[2]["offset"], 4)

    def test_disassemble_with_offset(self):
        bytecode = bytes([0x00, 0x00, 0x00, 0x00, 0x0E, 0x00])
        instructions = disassemble(bytecode, offset=2)
        self.assertEqual(len(instructions), 2)
        self.assertEqual(instructions[0]["offset"], 2)
        self.assertEqual(instructions[0]["mnemonic"], "nop")
        self.assertEqual(instructions[1]["offset"], 4)
        self.assertEqual(instructions[1]["mnemonic"], "return-void")

    def test_linear_sweep_const_strings(self):
        # Same hex as Rust test_linear_sweep_strings (first few instructions)
        hex_str = (
            "1A000F001A0100001A0214001A0311001A0415001A0413001A0508001A061200"
            "1A0716001A081000620900006E2002000900620000006E200200100062000000"
        )
        bytecode = bytes.fromhex(hex_str)
        instructions = disassemble(bytecode)
        self.assertGreater(len(instructions), 5)
        self.assertEqual(instructions[0]["mnemonic"], "const-string")
        self.assertEqual(instructions[10]["mnemonic"], "sget-object")
        self.assertEqual(instructions[11]["mnemonic"], "invoke-virtual")

    def test_offset_past_end(self):
        bytecode = bytes([0x00, 0x00])
        with self.assertRaises(ValueError):
            disassemble(bytecode, offset=5)

    def test_invalid_bytecode_stops(self):
        # Valid nop, then invalid 0xFF 0xAB (incomplete)
        bytecode = bytes([0x00, 0x00, 0xFF, 0xAB])
        with self.assertRaises(ValueError):
            disassemble(bytecode)

    def test_packed_switch_payload(self):
        # packed-switch v2, +0; padding nop; payload
        hex_str = (
            "2B02140000001300110038030400130063000F001300170028F913002A0028F6"
            "1300480028F3000000010300010000000A0000000D00000010000000"
        )
        bytecode = bytes.fromhex(hex_str)
        instructions = disassemble(bytecode)
        mnemonics = [ins["mnemonic"] for ins in instructions]
        self.assertIn("packed-switch", mnemonics)
        self.assertIn("packed-switch-payload", mnemonics)


class TestInstructionShape(unittest.TestCase):
    def test_all_instruction_dicts_have_same_keys(self):
        bytecode = bytes([0x00, 0x00, 0x01, 0x21, 0x0E, 0x00])
        instructions = disassemble(bytecode)
        keys = set(instructions[0].keys())
        for ins in instructions:
            self.assertEqual(set(ins.keys()), keys)
            self.assertIsInstance(ins["offset"], int)
            self.assertIsInstance(ins["length"], int)
            self.assertIsInstance(ins["opcode"], int)
            self.assertIsInstance(ins["mnemonic"], str)
            self.assertIsInstance(ins["operands"], str)
            self.assertIsInstance(ins["disasm"], str)


class TestConstInstructions(unittest.TestCase):
    """Mirror Rust tests for const/4, const/16, const-wide."""

    def test_const_4(self):
        # const/4 v0, 0
        bytecode = bytes([0x12, 0x00])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "const/4")
        self.assertEqual(ins["operands"], "v0, 0")

    def test_const_4_negative(self):
        # const/4 v0, -1 (0xF0)
        bytecode = bytes([0x12, 0xF0])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "const/4")
        self.assertIn("-1", ins["operands"])

    def test_const_16(self):
        bytecode = bytes([0x13, 0x00, 0x02, 0x20])  # const/16 v0, 0x2002
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "const/16")
        self.assertEqual(ins["length"], 4)

    def test_const_wide(self):
        # const-wide v0, 0 (10 bytes)
        bytecode = bytes([0x18, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["mnemonic"], "const-wide")
        self.assertEqual(ins["operands"], "v0, 0")
        self.assertEqual(ins["length"], 10)


class TestDisasmField(unittest.TestCase):
    def test_disasm_trimmed(self):
        bytecode = bytes([0x00, 0x00])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["disasm"], "nop")

    def test_disasm_with_operands(self):
        bytecode = bytes([0x01, 0x21])
        ins = decode_instruction(bytecode)
        self.assertEqual(ins["disasm"], "move v1, v2")


class TestFillArrayData(unittest.TestCase):
    def test_fill_array_data_payload_in_sequence(self):
        # Sequence that includes fill-array-data then payload (simplified: just check decode doesn't crash)
        hex_str = (
            "12412310030026002D0000005B30000012702300050026002B0000005B300300"
            "1250230004002600350000005B300100231007002600380000005B3002001220"
        )
        bytecode = bytes.fromhex(hex_str)
        instructions = disassemble(bytecode)
        self.assertGreater(len(instructions), 1)
        mnemonics = [ins["mnemonic"] for ins in instructions]
        self.assertIn("const/4", mnemonics)
        self.assertIn("fill-array-data", mnemonics)


if __name__ == "__main__":
    unittest.main()
