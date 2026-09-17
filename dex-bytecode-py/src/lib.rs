//! Python bindings for dex-bytecode: Dalvik instruction decoder, CFG, and patch helpers.
//!
//! Usage:
//!   from dex_bytecode_py import disassemble, basic_blocks, cfg_edges, patch_branch_target

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PySet};

use dex_bytecode::{
    basic_blocks as rust_basic_blocks, branch_target_offsets, cfg_edges as rust_cfg_edges,
    decode_all, decode_one, encode_goto, encode_instruction as rust_encode_instruction,
    encode_nop, encode_return_void, exception_edges as rust_exception_edges,
    opcode_for_mnemonic, patch_branch_target, IndexResolve, Instruction, TryCatchEntry,
};

fn map_err(e: impl ToString) -> PyErr {
    PyValueError::new_err(e.to_string())
}

/// Disassemble raw Dalvik bytecode.
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
        Err(e) => Err(map_err(e)),
    }
}

/// Decode a single instruction at the given offset.
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
        Err(e) => Err(map_err(e)),
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

/// Return branch target offsets (relative to the decoded region).
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
        Err(e) => Err(map_err(e)),
    }
}

/// Split decoded bytecode into basic blocks.
///
/// Returns a list of dicts with keys:
/// ``start_offset``, ``end_offset``, ``successors``, ``fallthrough_to``.
#[pyfunction]
#[pyo3(signature = (data, offset=0))]
fn basic_blocks(py: Python<'_>, data: &[u8], offset: usize) -> PyResult<Vec<PyObject>> {
    if offset >= data.len() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    let instructions = decode_all(&data[offset..], 0).map_err(map_err)?;
    let blocks = rust_basic_blocks(&instructions, data, offset);
    let mut out = Vec::with_capacity(blocks.len());
    for b in blocks {
        let dict = PyDict::new_bound(py);
        dict.set_item("start_offset", b.start_offset)?;
        // Last block uses u32::MAX as sentinel; expose concrete end for Python callers.
        let end = if b.end_offset == u32::MAX {
            instructions
                .last()
                .map(|ins| (ins.offset as u32) + (offset as u32) + ins.length() as u32)
                .unwrap_or(b.start_offset)
        } else {
            b.end_offset
        };
        dict.set_item("end_offset", end)?;
        dict.set_item("successors", b.successors)?;
        dict.set_item("fallthrough_to", b.fallthrough_to)?;
        out.push(dict.into_py(py));
    }
    Ok(out)
}

/// Return CFG edges ``(from_offset, to_offset)`` including fallthrough.
#[pyfunction]
#[pyo3(signature = (data, offset=0))]
fn cfg_edges(py: Python<'_>, data: &[u8], offset: usize) -> PyResult<Vec<PyObject>> {
    if offset >= data.len() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    let instructions = decode_all(&data[offset..], 0).map_err(map_err)?;
    let edges = rust_cfg_edges(&instructions, data, offset);
    let mut out = Vec::with_capacity(edges.len());
    for (from, to) in edges {
        let dict = PyDict::new_bound(py);
        dict.set_item("from", from)?;
        dict.set_item("to", to)?;
        out.push(dict.into_py(py));
    }
    Ok(out)
}

/// Rewrite the branch at ``from_offset`` so it jumps to ``to_offset``.
///
/// Returns the mutated bytecode as ``bytes``.
#[pyfunction]
fn patch_branch(data: &[u8], from_offset: usize, to_offset: u32) -> PyResult<Vec<u8>> {
    let mut buf = data.to_vec();
    patch_branch_target(&mut buf, from_offset, to_offset).map_err(map_err)?;
    Ok(buf)
}

#[pyfunction]
fn encode_nop_bytes() -> Vec<u8> {
    encode_nop().to_vec()
}

#[pyfunction]
fn encode_return_void_bytes() -> Vec<u8> {
    encode_return_void().to_vec()
}

#[pyfunction]
fn encode_goto_bytes(rel_units: i8) -> Vec<u8> {
    encode_goto(rel_units).to_vec()
}

/// Encode one instruction from mnemonic + operand text (index refs like ``method@33``).
#[pyfunction]
#[pyo3(signature = (mnemonic, operands="", branch_rel_units=None))]
fn encode_instruction(
    mnemonic: &str,
    operands: &str,
    branch_rel_units: Option<i32>,
) -> PyResult<Vec<u8>> {
    rust_encode_instruction(mnemonic, operands, &IndexResolve, branch_rel_units).map_err(map_err)
}

/// Look up the opcode byte for a mnemonic (or ``None`` if unknown).
#[pyfunction]
fn opcode_of(mnemonic: &str) -> Option<u8> {
    opcode_for_mnemonic(mnemonic)
}

/// Exception edges from try/catch ranges into basic-block starts.
///
/// Each entry is ``(start_offset, end_offset, handler_offset, type_index|None)``.
#[pyfunction]
#[pyo3(signature = (data, try_entries, offset=0))]
fn exception_edges(
    py: Python<'_>,
    data: &[u8],
    try_entries: Vec<(u32, u32, u32, Option<u32>)>,
    offset: usize,
) -> PyResult<Vec<PyObject>> {
    if offset >= data.len() && !data.is_empty() {
        return Err(PyValueError::new_err("offset past end of data"));
    }
    let instructions = if data.is_empty() {
        Vec::new()
    } else {
        decode_all(&data[offset..], 0).map_err(map_err)?
    };
    let blocks = rust_basic_blocks(&instructions, data, offset);
    let entries: Vec<TryCatchEntry> = try_entries
        .into_iter()
        .map(|(start_offset, end_offset, handler_offset, type_index)| TryCatchEntry {
            start_offset,
            end_offset,
            handler_offset,
            type_index,
        })
        .collect();
    let edges = rust_exception_edges(&entries, &blocks);
    let mut out = Vec::with_capacity(edges.len());
    for (from, to) in edges {
        let dict = PyDict::new_bound(py);
        dict.set_item("from", from)?;
        dict.set_item("to", to)?;
        out.push(dict.into_py(py));
    }
    Ok(out)
}

#[pymodule]
fn dex_bytecode_py(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_function(wrap_pyfunction!(disassemble, m)?)?;
    m.add_function(wrap_pyfunction!(decode_instruction, m)?)?;
    m.add_function(wrap_pyfunction!(get_branch_targets, m)?)?;
    m.add_function(wrap_pyfunction!(basic_blocks, m)?)?;
    m.add_function(wrap_pyfunction!(cfg_edges, m)?)?;
    m.add_function(wrap_pyfunction!(patch_branch, m)?)?;
    m.add_function(wrap_pyfunction!(encode_nop_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode_return_void_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode_goto_bytes, m)?)?;
    m.add_function(wrap_pyfunction!(encode_instruction, m)?)?;
    m.add_function(wrap_pyfunction!(opcode_of, m)?)?;
    m.add_function(wrap_pyfunction!(exception_edges, m)?)?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}
