//! Python bindings for dex-bytecode: Dalvik instruction decoder.
//!
//! Usage:
//!   from dex_bytecode_py import disassemble, Instruction
//!   instructions = disassemble(bytecode_bytes)
//!   for ins in instructions:
//!       print(ins["offset"], ins["mnemonic"], ins["operands"])

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PySet};

use dex_bytecode::{branch_target_offsets, decode_all, decode_one, Instruction};

/// Disassemble raw Dalvik bytecode.
///
/// Args:
///     data: bytes – Raw instruction bytes (e.g. from a DEX code_item).
///     offset: int = 0 – Start offset in bytes.
///
/// Returns:
///     list[dict]: List of instruction dicts with keys:
///         offset (int), length (int), opcode (int), mnemonic (str), operands (str).
///
/// Raises:
///     ValueError: On invalid or truncated bytecode.
#[pyfunction]
#[pyo3(signature = (data, offset=0))]
fn disassemble(py: Python<'_>, data: &[u8], offset: usize) -> PyResult<Vec<PyObject>> {
    if offset >= data.len() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    match decode_all(&data[offset..], 0) {
        Ok(instructions) => {
            let mut out = Vec::with_capacity(instructions.len());
            for ins in instructions {
                let dict = instruction_to_dict(py, &ins, offset)?;
                out.push(dict.into_py(py));
            }
            Ok(out)
        }
        Err(e) => Err(PyValueError::new_err(e.to_string())),
    }
}

/// Decode a single instruction at the given offset.
///
/// Args:
///     data: bytes – Raw instruction bytes.
///     offset: int = 0 – Byte offset of the instruction.
///
/// Returns:
///     dict: One instruction with keys offset, length, opcode, mnemonic, operands.
///
/// Raises:
///     ValueError: On invalid or truncated bytecode.
#[pyfunction]
#[pyo3(signature = (data, offset=0))]
fn decode_instruction(py: Python<'_>, data: &[u8], offset: usize) -> PyResult<PyObject> {
    if offset >= data.len() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    match decode_one(&data[offset..], 0) {
        Ok(ins) => {
            let dict = instruction_to_dict(py, &ins, offset)?;
            Ok(dict.into_py(py))
        }
        Err(e) => Err(PyValueError::new_err(e.to_string())),
    }
}

fn instruction_to_dict(
    py: Python<'_>,
    ins: &Instruction,
    base_offset: usize,
) -> PyResult<Py<PyDict>> {
    let dict = PyDict::new_bound(py);
    dict.set_item("offset", (ins.offset as usize) + base_offset)?;
    dict.set_item("length", ins.length())?;
    dict.set_item("opcode", ins.opcode())?;
    dict.set_item("mnemonic", ins.mnemonic())?;
    dict.set_item("operands", ins.operands())?;
    dict.set_item(
        "disasm",
        format!("{} {}", ins.mnemonic(), ins.operands()).trim_end(),
    )?;
    Ok(dict.unbind())
}

/// Return branch target offsets (relative to the decoded region) for bytecode starting at offset.
///
/// Args:
///     data: bytes – Raw instruction bytes.
///     offset: int = 0 – Start offset in bytes.
///
/// Returns:
///     set[int]: Byte offsets where branch targets land (for displaying labels).
///
/// Raises:
///     ValueError: On invalid or truncated bytecode.
#[pyfunction]
#[pyo3(signature = (data, offset=0))]
fn get_branch_targets(py: Python<'_>, data: &[u8], offset: usize) -> PyResult<Py<PySet>> {
    if offset >= data.len() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    match branch_target_offsets(data, offset) {
        Ok(targets) => {
            let set = PySet::empty_bound(py)?;
            for t in targets {
                set.add(t)?;
            }
            Ok(set.unbind())
        }
        Err(e) => Err(PyValueError::new_err(e.to_string())),
    }
}

/// dex_bytecode_py: Dalvik instruction decoder for Python.
#[pymodule]
fn dex_bytecode_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(disassemble, m)?)?;
    m.add_function(wrap_pyfunction!(decode_instruction, m)?)?;
    m.add_function(wrap_pyfunction!(get_branch_targets, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
