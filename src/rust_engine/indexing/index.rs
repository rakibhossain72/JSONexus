//! Per-field index structures.
//!
//! * `HashIndex`  — Python dict from field value to a set of document ids, so
//!   hashing/equality use CPython's own implementation (and raise the same
//!   `TypeError` for unhashable values).
//! * `RangeIndex` — sorted `(value, id)` list, ordered with Python's own sort
//!   so mixed-type fields raise exactly the same `TypeError` as the original.

use std::collections::HashSet;

use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PySet, PyString, PyTuple};

use crate::query::evaluator::path_get;
use crate::query::operators::rich_compare;
use crate::utils::types::DocumentId;

fn is_type_error(py: Python<'_>, err: &PyErr) -> bool {
    err.is_instance_of::<PyTypeError>(py)
}

fn py_str(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

fn document_id(document: &Bound<'_, PyAny>) -> PyResult<String> {
    let dict = document
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
    match dict.get_item("_id")? {
        Some(id) => py_str(&id),
        None => Err(PyKeyError::new_err("_id")),
    }
}

// -- hash index --------------------------------------------------------------

#[pyclass(module = "jsonexus.indexing.index", name = "HashIndex")]
pub struct HashIndex {
    #[pyo3(get)]
    pub field_path: String,
    map: Py<PyDict>,
    dirty: HashSet<DocumentId>,
    built: bool,
}

impl HashIndex {
    pub fn new_index(py: Python<'_>, field_path: String) -> Self {
        Self {
            field_path,
            map: PyDict::new(py).unbind(),
            dirty: HashSet::new(),
            built: false,
        }
    }

    pub fn is_built(&self) -> bool {
        self.built
    }

    pub fn get_ids(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<HashSet<DocumentId>> {
        let mut out = HashSet::new();
        if let Some(set) = self.map.bind(py).get_item(value)? {
            let set = set
                .cast::<PySet>()
                .map_err(|_| PyTypeError::new_err("Index entry is not a set"))?;
            for id in set.iter() {
                out.insert(py_str(&id)?);
            }
        }
        Ok(out)
    }

    pub fn rebuild_index(
        &mut self,
        py: Python<'_>,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.map = PyDict::new(py).unbind();
        self.dirty.clear();
        for document in documents.try_iter()? {
            self.add(py, &document?)?;
        }
        self.built = true;
        Ok(())
    }
}

#[pymethods]
impl HashIndex {
    #[new]
    fn new(py: Python<'_>, field_path: String) -> Self {
        Self::new_index(py, field_path)
    }

    #[getter]
    fn _built(&self) -> bool {
        self.built
    }

    fn add(&mut self, py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = match path_get(document, &self.field_path)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let map = self.map.bind(py);
        // Mirrors `_hashable()`: unhashable values are silently skipped.
        let existing = match map.get_item(value.bind(py)) {
            Ok(existing) => existing,
            Err(err) if is_type_error(py, &err) => return Ok(()),
            Err(err) => return Err(err),
        };
        let id = document_id(document)?;

        match existing {
            Some(set) => {
                set.cast::<PySet>()?.add(id.clone())?;
            }
            None => {
                let set = PySet::empty(py)?;
                set.add(id.clone())?;
                map.set_item(value.bind(py), set)?;
            }
        }
        self.dirty.remove(&id);
        Ok(())
    }

    fn remove(&mut self, py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = match path_get(document, &self.field_path)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let map = self.map.bind(py);
        let existing = match map.get_item(value.bind(py)) {
            Ok(existing) => existing,
            Err(err) if is_type_error(py, &err) => return Ok(()),
            Err(err) => return Err(err),
        };
        let id = document_id(document)?;

        if let Some(set) = existing {
            let set = set.cast::<PySet>()?;
            set.discard(id.clone())?;
            if set.len() == 0 {
                map.del_item(value.bind(py))?;
            }
        }
        self.dirty.remove(&id);
        Ok(())
    }

    fn update(
        &mut self,
        py: Python<'_>,
        old: &Bound<'_, PyAny>,
        new: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.remove(py, old)?;
        self.add(py, new)
    }

    fn invalidate(&mut self, doc_id: &Bound<'_, PyAny>) -> PyResult<()> {
        self.dirty.insert(py_str(doc_id)?);
        Ok(())
    }

    pub fn invalidate_all(&mut self, py: Python<'_>) {
        self.map = PyDict::new(py).unbind();
        self.dirty.clear();
        self.built = false;
    }

    fn rebuild(&mut self, py: Python<'_>, documents: &Bound<'_, PyAny>) -> PyResult<()> {
        self.rebuild_index(py, documents)
    }

    fn get(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PySet>> {
        let ids = self.get_ids(py, value)?;
        let set = PySet::empty(py)?;
        for id in ids {
            set.add(id)?;
        }
        Ok(set.unbind())
    }

    fn is_clean(&self, doc_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        Ok(!self.dirty.contains(&py_str(doc_id)?))
    }
}

// -- range index -------------------------------------------------------------

fn comparable(value: &Bound<'_, PyAny>) -> bool {
    if value.is_instance_of::<PyBool>() {
        return false;
    }
    value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>()
        || value.is_instance_of::<PyString>()
}

fn entry_value<'py>(
    entries: &Bound<'py, PyList>,
    index: usize,
) -> PyResult<Bound<'py, PyAny>> {
    entries.get_item(index)?.get_item(0)
}

fn is_lt(a: &Bound<'_, PyAny>, b: &Bound<'_, PyAny>) -> PyResult<bool> {
    rich_compare(a, b, CompareOp::Lt)
}

fn bisect_left(entries: &Bound<'_, PyList>, key: &Bound<'_, PyAny>) -> PyResult<usize> {
    let (mut lo, mut hi) = (0usize, entries.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if is_lt(&entry_value(entries, mid)?, key)? {
            lo = mid + 1;
        } else {
            hi = mid;
        }
    }
    Ok(lo)
}

fn bisect_right(entries: &Bound<'_, PyList>, key: &Bound<'_, PyAny>) -> PyResult<usize> {
    let (mut lo, mut hi) = (0usize, entries.len());
    while lo < hi {
        let mid = (lo + hi) / 2;
        if is_lt(key, &entry_value(entries, mid)?)? {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    Ok(lo)
}

#[pyclass(module = "jsonexus.indexing.index", name = "RangeIndex")]
pub struct RangeIndex {
    #[pyo3(get)]
    pub field_path: String,
    entries: Py<PyList>,
    built: bool,
}

impl RangeIndex {
    pub fn new_index(py: Python<'_>, field_path: String) -> Self {
        Self {
            field_path,
            entries: PyList::empty(py).unbind(),
            built: false,
        }
    }

    pub fn is_built(&self) -> bool {
        self.built
    }

    pub fn rebuild_index(
        &mut self,
        py: Python<'_>,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let entries = PyList::empty(py);
        for document in documents.try_iter()? {
            let document = document?;
            let value = match path_get(&document, &self.field_path)? {
                Some(value) => value,
                None => continue,
            };
            if !comparable(value.bind(py)) {
                continue;
            }
            let id = document_id(&document)?;
            let id_obj = PyString::new(py, &id);
            let tuple = PyTuple::new(py, [value, id_obj.into_any().unbind()])?;
            entries.append(tuple)?;
        }

        let operator = py.import("operator")?;
        let key = operator.getattr("itemgetter")?.call1((0,))?;
        let kwargs = PyDict::new(py);
        kwargs.set_item("key", key)?;
        entries.call_method("sort", (), Some(&kwargs))?;

        self.entries = entries.unbind();
        self.built = true;
        Ok(())
    }

    pub fn range_ids(
        &self,
        py: Python<'_>,
        low: Option<&Bound<'_, PyAny>>,
        high: Option<&Bound<'_, PyAny>>,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> PyResult<Vec<DocumentId>> {
        let entries = self.entries.bind(py);
        if entries.len() == 0 {
            return Ok(Vec::new());
        }

        let start = match low {
            Some(low) if !low.is_none() => {
                if inclusive_low {
                    bisect_left(entries, low)?
                } else {
                    bisect_right(entries, low)?
                }
            }
            _ => 0,
        };
        let end = match high {
            Some(high) if !high.is_none() => {
                if inclusive_high {
                    bisect_right(entries, high)?
                } else {
                    bisect_left(entries, high)?
                }
            }
            _ => entries.len(),
        };
        if start >= end {
            return Ok(Vec::new());
        }

        let mut ids = Vec::with_capacity(end - start);
        for index in start..end {
            let id = entries.get_item(index)?.get_item(1)?;
            ids.push(py_str(&id)?);
        }
        Ok(ids)
    }
}

#[pymethods]
impl RangeIndex {
    #[new]
    fn new(py: Python<'_>, field_path: String) -> Self {
        Self::new_index(py, field_path)
    }

    #[getter]
    fn _built(&self) -> bool {
        self.built
    }

    fn rebuild(&mut self, py: Python<'_>, documents: &Bound<'_, PyAny>) -> PyResult<()> {
        self.rebuild_index(py, documents)
    }

    pub fn invalidate_all(&mut self, py: Python<'_>) {
        self.entries = PyList::empty(py).unbind();
        self.built = false;
    }

    #[pyo3(signature = (low=None, high=None, inclusive_low=true, inclusive_high=true))]
    fn range(
        &self,
        py: Python<'_>,
        low: Option<Py<PyAny>>,
        high: Option<Py<PyAny>>,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> PyResult<Py<PyList>> {
        let low_bound = low.as_ref().map(|v| v.bind(py));
        let high_bound = high.as_ref().map(|v| v.bind(py));
        let ids = self.range_ids(py, low_bound, high_bound, inclusive_low, inclusive_high)?;
        let out: Vec<Py<PyAny>> = ids
            .iter()
            .map(|id| PyString::new(py, id).into_any().unbind())
            .collect();
        Ok(PyList::new(py, out)?.unbind())
    }
}
