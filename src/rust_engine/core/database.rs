//! Top-level database coordinator.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};

use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

use crate::collections::manager::{
    self as collection_impl, state_from_list, Collection, CollectionState,
};
use crate::storage::backend::{path_from_python, Backend, BackendImpl, FileBackend, PythonBackend};
use crate::utils::validation::validate_collection_name;

fn lock_state(state: &Arc<Mutex<CollectionState>>) -> PyResult<MutexGuard<'_, CollectionState>> {
    state
        .lock()
        .map_err(|_| PyRuntimeError::new_err("collection lock poisoned"))
}

fn simple_dict(py: Python<'_>, pairs: &[(&str, Py<PyAny>)]) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    for (key, value) in pairs {
        out.set_item(*key, value)?;
    }
    Ok(out.into_any().unbind())
}

fn py_text(py: Python<'_>, text: &str) -> Py<PyAny> {
    PyString::new(py, text).into_any().unbind()
}

/// Top-level coordinator for a JSONexus database.
#[pyclass(module = "jsonexus.core.database", name = "JSONexus")]
pub struct JSONexus {
    #[pyo3(get, set)]
    pub db_name: Py<PyAny>,
    backend: BackendImpl,
    raw: Py<PyDict>,
    collections: HashMap<String, Py<Collection>>,
}

impl JSONexus {
    fn get_collection_impl(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Collection>> {
        validate_collection_name(collection_name)?;
        let key: String = collection_name.extract()?;

        if let Some(cached) = self.collections.get(&key) {
            return Ok(cached.clone_ref(py));
        }

        let raw = self.raw.bind(py);
        let raw_list = match raw.get_item(collection_name)? {
            Some(value) => value,
            None => {
                return Err(PyKeyError::new_err(collection_name.clone().unbind()));
            }
        };
        let list = raw_list
            .cast::<PyList>()
            .map_err(|_| PyTypeError::new_err("Collection must be a list"))?
            .clone()
            .unbind();

        let state = state_from_list(py, key.clone(), list)?;
        let collection = Py::new(py, Collection::new_state(state))?;
        self.collections.insert(key, collection.clone_ref(py));
        Ok(collection)
    }

    fn ensure_collection_impl(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Collection>> {
        let raw = self.raw.bind(py);
        if !raw.contains(collection_name)? {
            raw.set_item(collection_name, PyList::empty(py))?;
        }
        self.get_collection_impl(py, collection_name)
    }

    fn state_for(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Arc<Mutex<CollectionState>>> {
        let collection = self.get_collection_impl(py, collection_name)?;
        let state = collection.borrow(py).state.clone();
        Ok(state)
    }

    fn ensure_state(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Arc<Mutex<CollectionState>>> {
        let collection = self.ensure_collection_impl(py, collection_name)?;
        let state = collection.borrow(py).state.clone();
        Ok(state)
    }

    fn persist(&self, py: Python<'_>) -> PyResult<()> {
        self.backend.save(py, self.raw.bind(py).as_any())
    }

    fn mutate_collection<F>(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        operation: F,
    ) -> PyResult<Py<PyAny>>
    where
        F: FnOnce(Python<'_>, &mut CollectionState) -> PyResult<Py<PyAny>>,
    {
        let state = self.ensure_state(py, collection_name)?;
        let result = {
            let mut guard = lock_state(&state)?;
            operation(py, &mut guard)?
        };
        self.persist(py)?;
        Ok(result)
    }

    fn mutate_existing<F>(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        operation: F,
    ) -> PyResult<Py<PyAny>>
    where
        F: FnOnce(Python<'_>, &mut CollectionState) -> PyResult<Py<PyAny>>,
    {
        let state = self.state_for(py, collection_name)?;
        let result = {
            let mut guard = lock_state(&state)?;
            operation(py, &mut guard)?
        };
        self.persist(py)?;
        Ok(result)
    }

    fn read_collection<F, R>(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        operation: F,
    ) -> PyResult<R>
    where
        F: FnOnce(Python<'_>, &mut CollectionState) -> PyResult<R>,
    {
        let state = self.state_for(py, collection_name)?;
        let mut guard = lock_state(&state)?;
        operation(py, &mut guard)
    }
}

#[pymethods]
impl JSONexus {
    #[new]
    #[pyo3(signature = (db_name, storage_backend=None))]
    fn new(
        py: Python<'_>,
        db_name: &Bound<'_, PyAny>,
        storage_backend: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let backend = match storage_backend {
            Some(backend) => BackendImpl::Py(PythonBackend {
                obj: backend.clone().unbind(),
            }),
            None => BackendImpl::File(FileBackend {
                path: path_from_python(db_name)?,
            }),
        };

        let loaded = backend.load(py)?;
        let raw = loaded
            .bind(py)
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("Database root must be a dict"))?
            .clone()
            .unbind();

        Ok(Self {
            db_name: db_name.clone().unbind(),
            backend,
            raw,
            collections: HashMap::new(),
        })
    }

    // -- internal -----------------------------------------------------------

    fn _get_collection(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Collection>> {
        self.get_collection_impl(py, collection_name)
    }

    fn _ensure_collection(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<Collection>> {
        self.ensure_collection_impl(py, collection_name)
    }

    fn _persist(&self, py: Python<'_>) -> PyResult<()> {
        self.persist(py)
    }

    // -- public API ---------------------------------------------------------

    fn insert(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_collection(py, collection_name, |py, state| {
            collection_impl::insert(py, state, data)
        })
    }

    fn insert_no_duplicate(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_collection(py, collection_name, |py, state| {
            collection_impl::insert_no_duplicate(py, state, data)
        })
    }

    fn insert_many(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_collection(py, collection_name, |py, state| {
            collection_impl::insert_many(py, state, data)
        })
    }

    fn create_collection(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        validate_collection_name(collection_name)?;
        let raw = self.raw.bind(py);
        if raw.contains(collection_name)? {
            return simple_dict(
                py,
                &[
                    ("status", py_text(py, "failed")),
                    ("message", py_text(py, "Collection already exists")),
                ],
            );
        }
        raw.set_item(collection_name, PyList::empty(py))?;
        self.persist(py)?;
        simple_dict(
            py,
            &[
                ("status", py_text(py, "success")),
                ("name", collection_name.clone().unbind()),
            ],
        )
    }

    fn get_all(&self, py: Python<'_>) -> Py<PyAny> {
        self.raw.clone_ref(py).into_any()
    }

    fn get_collection(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyList>> {
        self.read_collection(py, collection_name, |py, state| {
            collection_impl::to_list(py, state)
        })
    }

    fn get_collections(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        Ok(self.raw.bind(py).keys().unbind())
    }

    fn drop_collection(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        validate_collection_name(collection_name)?;
        let key: String = collection_name.extract()?;
        let raw = self.raw.bind(py);
        if !raw.contains(collection_name)? {
            return simple_dict(
                py,
                &[
                    ("status", py_text(py, "failed")),
                    ("message", py_text(py, "Collection not found")),
                ],
            );
        }
        raw.del_item(collection_name)?;
        self.collections.remove(&key);
        self.persist(py)?;
        simple_dict(
            py,
            &[
                ("status", py_text(py, "success")),
                ("name", collection_name.clone().unbind()),
            ],
        )
    }

    fn find(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let result = self.read_collection(py, collection_name, |py, state| {
            collection_impl::find(py, state, query)
        })?;
        simple_dict(py, &[("result", result.into_any())])
    }

    fn find_one(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let document = self.read_collection(py, collection_name, |py, state| {
            collection_impl::find_one(py, state, query)
        })?;
        match document {
            Some(document) => Ok(document),
            None => Ok(PyList::empty(py).into_any().unbind()),
        }
    }

    fn count(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
    ) -> PyResult<usize> {
        self.read_collection(py, collection_name, |inner_py, state| {
            Ok(collection_impl::count(inner_py, state))
        })
    }

    fn get_document(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        document_id: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        self.read_collection(py, collection_name, |py, state| {
            collection_impl::get(py, state, document_id)
        })
    }

    #[pyo3(name = "match")]
    fn match_(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.read_collection(py, collection_name, |py, state| {
            collection_impl::match_documents(py, state, query)
        })
    }

    fn update(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
        update_fields: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_existing(py, collection_name, |py, state| {
            collection_impl::update(py, state, query, update_fields)
        })
    }

    fn updateby_id(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        document_id: &Bound<'_, PyAny>,
        update_fields: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_existing(py, collection_name, |py, state| {
            collection_impl::update_by_id(py, state, document_id, update_fields)
        })
    }

    fn delete(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_existing(py, collection_name, |py, state| {
            collection_impl::delete(py, state, query)
        })
    }

    fn deleteby_id(
        &mut self,
        py: Python<'_>,
        collection_name: &Bound<'_, PyAny>,
        document_id: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        self.mutate_existing(py, collection_name, |py, state| {
            collection_impl::delete_by_id(py, state, document_id)
        })
    }

    // -- lifecycle ----------------------------------------------------------

    fn flush(&self, py: Python<'_>) -> PyResult<()> {
        self.persist(py)
    }

    fn close(&self, py: Python<'_>) -> PyResult<()> {
        self.persist(py)
    }

    fn __enter__(slf: PyRef<'_, Self>) -> PyRef<'_, Self> {
        slf
    }

    fn __exit__(
        &self,
        py: Python<'_>,
        _exc_type: &Bound<'_, PyAny>,
        _exc_value: &Bound<'_, PyAny>,
        _traceback: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.persist(py)
    }
}
