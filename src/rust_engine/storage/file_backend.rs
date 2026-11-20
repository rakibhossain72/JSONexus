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

const FLUSH_AT: usize = 1 << 20;
const SPACES: &[u8] = b"                                                                ";
const HEX: &[u8; 16] = b"0123456789abcdef";

/// Buffered writer that keeps only ~1 MiB of serialized JSON in memory. At
/// multi-gigabyte database sizes this replaces a full-size `Vec<u8>` payload
/// with a stream straight to the temporary file.
struct JsonSink<'a> {
    file: &'a mut fs::File,
    buffer: Vec<u8>,
}

impl<'a> JsonSink<'a> {
    fn new(file: &'a mut fs::File) -> Self {
        Self {
            file,
            buffer: Vec::with_capacity(FLUSH_AT),
        }
    }

    fn write(&mut self, bytes: &[u8]) -> PyResult<()> {
        if self.buffer.len() + bytes.len() > FLUSH_AT {
            self.flush()?;
            if bytes.len() >= FLUSH_AT {
                self.file.write_all(bytes)?;
                return Ok(());
            }
        }
        self.buffer.extend_from_slice(bytes);
        Ok(())
    }

    fn byte(&mut self, byte: u8) -> PyResult<()> {
        if self.buffer.len() == FLUSH_AT {
            self.flush()?;
        }
        self.buffer.push(byte);
        Ok(())
    }

    fn flush(&mut self) -> PyResult<()> {
        if !self.buffer.is_empty() {
            self.file.write_all(&self.buffer)?;
            self.buffer.clear();
        }
        Ok(())
    }
}

fn write_indent(sink: &mut JsonSink<'_>, level: usize) -> PyResult<()> {
    let mut remaining = level * 2;
    while remaining > 0 {
        let take = remaining.min(SPACES.len());
        sink.write(&SPACES[..take])?;
        remaining -= take;
    }
    Ok(())
}

fn write_json_string(sink: &mut JsonSink<'_>, text: &str) -> PyResult<()> {
    let bytes = text.as_bytes();
    sink.byte(b'"')?;
    let mut start = 0;
    for (index, &byte) in bytes.iter().enumerate() {
        let escape: Option<&[u8]> = match byte {
            b'"' => Some(b"\\\""),
            b'\\' => Some(b"\\\\"),
            0x08 => Some(b"\\b"),
            0x0c => Some(b"\\f"),
            b'\n' => Some(b"\\n"),
            b'\r' => Some(b"\\r"),
            b'\t' => Some(b"\\t"),
            0x00..=0x1f => None,
            _ => continue,
        };
        if start < index {
            sink.write(&bytes[start..index])?;
        }
        match escape {
            Some(sequence) => sink.write(sequence)?,
            None => {
                let sequence = [
                    b'\\',
                    b'u',
                    b'0',
                    b'0',
                    HEX[(byte >> 4) as usize],
                    HEX[(byte & 0x0f) as usize],
                ];
                sink.write(&sequence)?;
            }
        }
        start = index + 1;
    }
    if start < bytes.len() {
        sink.write(&bytes[start..])?;
    }
    sink.byte(b'"')
}

/// Write Python objects as 2-space-indented JSON, matching
/// `orjson.OPT_INDENT_2` output, directly into the file sink.
fn write_json(
    sink: &mut JsonSink<'_>,
    value: &Bound<'_, PyAny>,
    indent: usize,
) -> PyResult<()> {
    if value.is_none() {
        sink.write(b"null")?;
        return Ok(());
    }
    if let Ok(dict) = value.cast::<PyDict>() {
        let len = dict.len();
        if len == 0 {
            sink.write(b"{}")?;
            return Ok(());
        }
        sink.write(b"{\n")?;
        for (index, (key, item)) in dict.iter().enumerate() {
            write_indent(sink, indent + 1)?;
            let key = match key.cast::<PyString>() {
                Ok(key) => key.to_string_lossy(),
                Err(_) => return Err(PyTypeError::new_err("Dict key must be str")),
            };
            write_json_string(sink, &key)?;
            sink.write(b": ")?;
            write_json(sink, &item, indent + 1)?;
            if index + 1 < len {
                sink.byte(b',')?;
            }
            sink.byte(b'\n')?;
        }
        write_indent(sink, indent)?;
        sink.byte(b'}')?;
        return Ok(());
    }
    if let Ok(list) = value.cast::<PyList>() {
        return write_json_array(sink, list.as_any(), indent);
    }
    if let Ok(tuple) = value.cast::<PyTuple>() {
        return write_json_array(sink, tuple.as_any(), indent);
    }
    if let Ok(text) = value.cast::<PyString>() {
        write_json_string(sink, &text.to_string_lossy())?;
        return Ok(());
    }
    if let Ok(boolean) = value.cast::<PyBool>() {
        sink.write(if boolean.is_true() { b"true" } else { b"false" })?;
        return Ok(());
    }
    if let Ok(integer) = value.cast::<PyInt>() {
        if let Ok(n) = integer.extract::<i64>() {
            sink.write(n.to_string().as_bytes())?;
        } else if let Ok(n) = integer.extract::<u64>() {
            sink.write(n.to_string().as_bytes())?;
        } else {
            let text = integer.str()?;
            sink.write(text.to_string_lossy().as_bytes())?;
        }
        return Ok(());
    }
    if let Ok(float) = value.cast::<PyFloat>() {
        match Number::from_f64(float.value()) {
            Some(number) => sink.write(number.to_string().as_bytes())?,
            None => sink.write(b"null")?,
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
    sink: &mut JsonSink<'_>,
    values: &Bound<'_, PyAny>,
    indent: usize,
) -> PyResult<()> {
    let len = values.len()?;
    if len == 0 {
        sink.write(b"[]")?;
        return Ok(());
    }
    sink.write(b"[\n")?;
    for (index, item) in values.try_iter()?.enumerate() {
        let item = item?;
        write_indent(sink, indent + 1)?;
        write_json(sink, &item, indent + 1)?;
        if index + 1 < len {
            sink.byte(b',')?;
        }
        sink.byte(b'\n')?;
    }
    write_indent(sink, indent)?;
    sink.byte(b']')?;
    Ok(())
}

/// Serialize Python objects to pretty JSON and atomically replace `path`.
pub fn save_file(py: Python<'_>, path: &Path, data: &Bound<'_, PyAny>) -> PyResult<()> {
    let _ = py;
    let dir = match path.parent() {
        Some(p) if !p.as_os_str().is_empty() => p,
        _ => Path::new("."),
    };
    fs::create_dir_all(dir)?;

    let mut tmp = tempfile::Builder::new()
        .suffix(".tmp")
        .tempfile_in(dir)?;
    {
        let mut sink = JsonSink::new(tmp.as_file_mut());
        write_json(&mut sink, data, 0)?;
        sink.flush()?;
    }
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
