//! JSON file storage with atomic replacement.

use std::fs;
use std::io::Write;
use std::path::Path;

use pyo3::exceptions::{PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use pythonize::pythonize;
use serde_json::{Number, Value};

use crate::storage::backend::{path_from_python, StorageBackendBase};

/// Load a JSON file into Python objects. Missing/empty files yield `{}`.
pub fn load_file(py: Python<'_>, path: &Path) -> PyResult<Py<PyAny>> {
    if !path.exists() {
        return Ok(PyDict::new(py).into_any().unbind());
    }
    let raw = fs::read(path)?;
    if raw.is_empty() {
        return Ok(PyDict::new(py).into_any().unbind());
    }
    let value: Value =
        serde_json::from_slice(&raw).map_err(|e| PyValueError::new_err(e.to_string()))?;
    let obj = pythonize(py, &value).map_err(|e| PyValueError::new_err(e.to_string()))?;
    Ok(obj.unbind())
}

fn write_indent(out: &mut Vec<u8>, level: usize) {
    for _ in 0..level {
        out.extend_from_slice(b"  ");
    }
}

fn write_json_string(out: &mut Vec<u8>, text: &str) {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    out.push(b'"');
    for &byte in text.as_bytes() {
        match byte {
            b'"' => out.extend_from_slice(b"\\\""),
            b'\\' => out.extend_from_slice(b"\\\\"),
            0x08 => out.extend_from_slice(b"\\b"),
            0x0c => out.extend_from_slice(b"\\f"),
            b'\n' => out.extend_from_slice(b"\\n"),
            b'\r' => out.extend_from_slice(b"\\r"),
            b'\t' => out.extend_from_slice(b"\\t"),
            0x00..=0x1f => {
                out.extend_from_slice(b"\\u00");
                out.push(HEX[(byte >> 4) as usize]);
                out.push(HEX[(byte & 0x0f) as usize]);
            }
            _ => out.push(byte),
        }
    }
    out.push(b'"');
}

/// Write Python objects as 2-space-indented JSON, matching
/// `orjson.OPT_INDENT_2` output. Writing bytes directly avoids building an
/// intermediate `serde_json::Value` tree for every save.
fn write_json(out: &mut Vec<u8>, value: &Bound<'_, PyAny>, indent: usize) -> PyResult<()> {
    if value.is_none() {
        out.extend_from_slice(b"null");
        return Ok(());
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let len = dict.len();
        if len == 0 {
            out.extend_from_slice(b"{}");
            return Ok(());
        }
        out.extend_from_slice(b"{\n");
        for (index, (key, item)) in dict.iter().enumerate() {
            write_indent(out, indent + 1);
            let key = match key.cast::<PyString>() {
                Ok(key) => key.to_string_lossy(),
                Err(_) => return Err(PyTypeError::new_err("Dict key must be str")),
            };
            write_json_string(out, &key);
            out.extend_from_slice(b": ");
            write_json(out, &item, indent + 1)?;
            if index + 1 < len {
                out.push(b',');
            }
            out.push(b'\n');
        }
        write_indent(out, indent);
        out.push(b'}');
        return Ok(());
    }
    if let Ok(list) = value.cast::<PyList>() {
        return write_json_array(out, list.as_any(), indent);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return write_json_array(out, tuple.as_any(), indent);
    }
    if let Ok(text) = value.cast::<PyString>() {
        write_json_string(out, &text.to_string_lossy());
        return Ok(());
    }
    if let Ok(boolean) = value.cast::<PyBool>() {
        out.extend_from_slice(if boolean.is_true() { b"true" } else { b"false" });
        return Ok(());
    }
    if let Ok(integer) = value.cast::<PyInt>() {
        if let Ok(n) = integer.extract::<i64>() {
            out.extend_from_slice(n.to_string().as_bytes());
        } else if let Ok(n) = integer.extract::<u64>() {
            out.extend_from_slice(n.to_string().as_bytes());
        } else {
            let text = integer.str()?;
            out.extend_from_slice(text.to_string_lossy().as_bytes());
        }
        return Ok(());
    }
    if let Ok(float) = value.cast::<PyFloat>() {
        match Number::from_f64(float.value()) {
            Some(number) => out.extend_from_slice(number.to_string().as_bytes()),
            None => out.extend_from_slice(b"null"),
        }
        return Ok(());
    }

    let type_name = value
        .get_type()
        .name()
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "object".to_string());
    Err(PyTypeError::new_err(format!(
        "Type is not JSON serializable: {type_name}"
    )))
}

fn write_json_array(
    out: &mut Vec<u8>,
    values: &Bound<'_, PyAny>,
    indent: usize,
) -> PyResult<()> {
    let len = values.len()?;
    if len == 0 {
        out.extend_from_slice(b"[]");
        return Ok(());
    }
    out.extend_from_slice(b"[\n");
    for (index, item) in values.try_iter()?.enumerate() {
        let item = item?;
        write_indent(out, indent + 1);
        write_json(out, &item, indent + 1)?;
        if index + 1 < len {
            out.push(b',');
        }
        out.push(b'\n');
    }
    write_indent(out, indent);
    out.push(b']');
    Ok(())
}

/// Serialize Python objects to pretty JSON and atomically replace `path`.
pub fn save_file(py: Python<'_>, path: &Path, data: &Bound<'_, PyAny>) -> PyResult<()> {
    let _ = py;
    let mut payload = Vec::new();
    write_json(&mut payload, data, 0)?;

    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)?;

    let mut tmp = tempfile::Builder::new()
        .suffix(".tmp")
        .tempfile_in(dir)?;
    tmp.write_all(&payload)?;
    tmp.persist(path).map_err(|e| PyErr::from(e.error))?;
    Ok(())
}

/// Storage backend that reads/writes a JSON file on disk.
#[pyclass(
    extends = StorageBackendBase,
    module = "jsonexus.storage.file_backend",
    name = "FileStorageBackend"
)]
pub struct FileStorageBackend {
    path: String,
}

#[pymethods]
impl FileStorageBackend {
    #[new]
    fn new(path: &Bound<'_, PyAny>) -> PyResult<PyClassInitializer<Self>> {
        let resolved = path_from_python(path)?;
        Ok(
            PyClassInitializer::from(StorageBackendBase).add_subclass(Self {
                path: resolved.to_string_lossy().into_owned(),
            }),
        )
    }

    #[getter]
    fn path(&self) -> String {
        self.path.clone()
    }

    fn load(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        load_file(py, Path::new(&self.path))
    }

    fn save(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        save_file(py, Path::new(&self.path), data)
    }

    fn exists(&self) -> PyResult<bool> {
        Ok(Path::new(&self.path).exists())
    }

    fn delete(&self) -> PyResult<()> {
        if Path::new(&self.path).exists() {
            fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}
