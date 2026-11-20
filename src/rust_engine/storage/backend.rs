//! Storage backend abstraction.
//!
//! Mirrors `jsonexus/storage/backend.py`: the Python-visible base class exists
//! so `isinstance` checks keep working, while the Rust trait is used
//! internally so a default file backend costs no Python call overhead.

use std::path::PathBuf;

use pyo3::exceptions::PyNotImplementedError;
use pyo3::prelude::*;

use crate::storage::file_backend::{load_file, save_file};

/// Rust-side storage backend trait.
#[allow(dead_code)]
pub trait Backend: Send + Sync {
    fn load(&self, py: Python<'_>) -> PyResult<Py<PyAny>>;
    fn save(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()>;
    fn exists(&self) -> PyResult<bool>;
    fn delete(&self) -> PyResult<()>;
}

/// A backend implemented as an arbitrary Python object with the same four
/// methods. This preserves the optional `storage_backend` constructor argument.
pub struct PythonBackend {
    pub obj: Py<PyAny>,
}

impl Backend for PythonBackend {
    fn load(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        Ok(self.obj.bind(py).call_method0("load")?.unbind())
    }

    fn save(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        self.obj.bind(py).call_method1("save", (data,))?;
        Ok(())
    }

    fn exists(&self) -> PyResult<bool> {
        Python::attach(|py| self.obj.bind(py).call_method0("exists")?.is_truthy())
    }

    fn delete(&self) -> PyResult<()> {
        Python::attach(|py| {
            self.obj.bind(py).call_method0("delete")?;
            Ok(())
        })
    }
}

/// The default file-backed backend.
pub struct FileBackend {
    pub path: PathBuf,
}

impl Backend for FileBackend {
    fn load(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        load_file(py, &self.path)
    }

    fn save(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        save_file(py, &self.path, data)
    }

    fn exists(&self) -> PyResult<bool> {
        Ok(self.path.exists())
    }

    fn delete(&self) -> PyResult<()> {
        if self.path.exists() {
            std::fs::remove_file(&self.path)?;
        }
        Ok(())
    }
}

/// Enum wrapper so `JSONexus` can hold either backend without dynamic dispatch.
pub enum BackendImpl {
    File(FileBackend),
    Py(PythonBackend),
}

impl Backend for BackendImpl {
    fn load(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        match self {
            BackendImpl::File(b) => b.load(py),
            BackendImpl::Py(b) => b.load(py),
        }
    }

    fn save(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<()> {
        match self {
            BackendImpl::File(b) => b.save(py, data),
            BackendImpl::Py(b) => b.save(py, data),
        }
    }

    fn exists(&self) -> PyResult<bool> {
        match self {
            BackendImpl::File(b) => b.exists(),
            BackendImpl::Py(b) => b.exists(),
        }
    }

    fn delete(&self) -> PyResult<()> {
        match self {
            BackendImpl::File(b) => b.delete(),
            BackendImpl::Py(b) => b.delete(),
        }
    }
}

/// Python-visible abstract base class (`StorageBackend`).
#[pyclass(subclass, module = "jsonexus.storage.backend", name = "StorageBackend")]
pub struct StorageBackendBase;

#[pymethods]
impl StorageBackendBase {
    fn load(&self) -> PyResult<Py<PyAny>> {
        Err(PyNotImplementedError::new_err(""))
    }

    fn save(&self, _data: &Bound<'_, PyAny>) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(""))
    }

    fn exists(&self) -> PyResult<bool> {
        Err(PyNotImplementedError::new_err(""))
    }

    fn delete(&self) -> PyResult<()> {
        Err(PyNotImplementedError::new_err(""))
    }
}

/// Resolve a Python path-like value the same way `open()` would.
pub fn path_from_python(value: &Bound<'_, PyAny>) -> PyResult<PathBuf> {
    let py = value.py();
    let os = py.import("os")?;
    let fspath = os.call_method1("fspath", (value,))?;
    let text: String = fspath.extract()?;
    Ok(PathBuf::from(text))
}
