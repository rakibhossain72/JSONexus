//! Per-field index structures.
//!
//! * `HashIndex`  — Python dict from field value to a set of document ids, so
//!   hashing/equality use CPython's own implementation (and raise the same
//!   `TypeError` for unhashable values).
//! * `RangeIndex` — sorted `(value, id)` list, ordered with Python's own sort
//!   so mixed-type fields raise exactly the same `TypeError` as the original.

use std::cmp::Ordering;
use std::collections::HashSet;

use pyo3::exceptions::{PyKeyError, PyTypeError};
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PySet, PyString};

use crate::query::evaluator::path_get;
use crate::utils::types::DocumentId;

fn is_type_error(py: Python<'_>, err: &PyErr) -> bool {
    err.is_instance_of::<PyTypeError>(py)
}

fn py_str(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

fn document_id_obj<'py>(document: &Bound<'py, PyAny>) -> PyResult<Bound<'py, PyAny>> {
    let dict = document
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
    match dict.get_item("_id")? {
        Some(id) => Ok(id),
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
    /// Store `document` under `value` in the inner id -> document mapping.
    pub fn insert_document(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        id: &Bound<'_, PyAny>,
        document: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let value = match value {
            Some(value) => value,
            None => return Ok(()),
        };
        let map = self.map.bind(py);
        let existing = match map.get_item(value) {
            Ok(existing) => existing,
            Err(err) if is_type_error(py, &err) => return Ok(()),
            Err(err) => return Err(err),
        };
        match existing {
            Some(inner) => {
                inner.cast::<PyDict>()?.set_item(id, document)?;
            }
            None => {
                let inner = PyDict::new(py);
                inner.set_item(id, document)?;
                map.set_item(value, inner)?;
            }
        }
        if !self.dirty.is_empty() {
            self.dirty.remove(&py_str(id)?);
        }
        Ok(())
    }

    /// Remove `id` from the bucket stored at `value` (if present).
    pub fn remove_value(
        &mut self,
        py: Python<'_>,
        value: Option<&Bound<'_, PyAny>>,
        id: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        let value = match value {
            Some(value) => value,
            None => return Ok(()),
        };
        let map = self.map.bind(py);
        let existing = match map.get_item(value) {
            Ok(existing) => existing,
            Err(err) if is_type_error(py, &err) => return Ok(()),
            Err(err) => return Err(err),
        };
        if let Some(inner) = existing {
            let inner = inner.cast::<PyDict>()?;
            if inner.contains(id)? {
                inner.del_item(id)?;
            }
            if inner.len() == 0 {
                map.del_item(value)?;
            }
        }
        if !self.dirty.is_empty() {
            self.dirty.remove(&py_str(id)?);
        }
        Ok(())
    }

    pub fn update_values(
        &mut self,
        py: Python<'_>,
        old_value: Option<&Bound<'_, PyAny>>,
        new_value: Option<&Bound<'_, PyAny>>,
        old_id: &Bound<'_, PyAny>,
        new_id: &Bound<'_, PyAny>,
        document: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        self.remove_value(py, old_value, old_id)?;
        self.insert_document(py, new_value, new_id, document)
    }

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

    /// Documents stored for an equality value, ready to return to Python.
    pub fn docs_for(
        &self,
        py: Python<'_>,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyList>> {
        match self.map.bind(py).get_item(value)? {
            Some(inner) => {
                let inner = inner
                    .cast::<PyDict>()
                    .map_err(|_| PyTypeError::new_err("Index entry is not a dict"))?;
                Ok(inner.values().unbind())
            }
            None => Ok(PyList::empty(py).unbind()),
        }
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

    pub fn add(&mut self, py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = match path_get(document, &self.field_path)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let id = document_id_obj(document)?;
        self.insert_document(py, Some(value.bind(py)), &id, document)
    }

    pub fn remove(&mut self, py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<()> {
        let value = match path_get(document, &self.field_path)? {
            Some(value) => value,
            None => return Ok(()),
        };
        let id = document_id_obj(document)?;
        self.remove_value(py, Some(value.bind(py)), &id)
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

    pub fn get(&self, py: Python<'_>, value: &Bound<'_, PyAny>) -> PyResult<Py<PySet>> {
        let set = PySet::empty(py)?;
        if let Some(inner) = self.map.bind(py).get_item(value)? {
            let inner = inner
                .cast::<PyDict>()
                .map_err(|_| PyTypeError::new_err("Index entry is not a dict"))?;
            for (id, _) in inner.iter() {
                set.add(id)?;
            }
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

#[derive(Clone)]
enum RangeKey {
    Int(i64),
    Float(f64),
    Str(String),
}

impl RangeKey {
    fn is_numeric(&self) -> bool {
        !matches!(self, RangeKey::Str(_))
    }

    fn compare(&self, other: &Self) -> Ordering {
        match (self, other) {
            (RangeKey::Int(a), RangeKey::Int(b)) => a.cmp(b),
            (RangeKey::Float(a), RangeKey::Float(b)) => {
                a.partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (RangeKey::Int(a), RangeKey::Float(b)) => {
                (*a as f64).partial_cmp(b).unwrap_or(Ordering::Equal)
            }
            (RangeKey::Float(a), RangeKey::Int(b)) => {
                a.partial_cmp(&(*b as f64)).unwrap_or(Ordering::Equal)
            }
            (RangeKey::Str(a), RangeKey::Str(b)) => a.cmp(b),
            // Mixed categories are rejected before sorting/searching.
            _ => Ordering::Equal,
        }
    }

    fn type_name(&self) -> &'static str {
        match self {
            RangeKey::Int(_) => "int",
            RangeKey::Float(_) => "float",
            RangeKey::Str(_) => "str",
        }
    }
}

struct RangeEntry {
    key: RangeKey,
    index: usize,
    id: Py<PyAny>,
}

fn value_key(value: &Bound<'_, PyAny>, allow_bool: bool) -> Option<RangeKey> {
    if value.is_instance_of::<PyBool>() {
        if !allow_bool {
            return None;
        }
        return Some(RangeKey::Int(if value.extract::<bool>().ok()? {
            1
        } else {
            0
        }));
    }
    if let Ok(text) = value.cast::<PyString>() {
        return Some(RangeKey::Str(text.to_string_lossy().into_owned()));
    }
    if let Ok(integer) = value.cast::<PyInt>() {
        if let Ok(number) = integer.extract::<i64>() {
            return Some(RangeKey::Int(number));
        }
        if let Ok(number) = integer.extract::<f64>() {
            return Some(RangeKey::Float(number));
        }
        return None;
    }
    if let Ok(float) = value.cast::<PyFloat>() {
        return Some(RangeKey::Float(float.value()));
    }
    None
}

fn operand_key(value: &Bound<'_, PyAny>) -> PyResult<RangeKey> {
    match value_key(value, true) {
        Some(key) => Ok(key),
        None => {
            let type_name = value
                .get_type()
                .name()
                .map(|n| n.to_string())
                .unwrap_or_else(|_| "object".to_string());
            Err(PyTypeError::new_err(format!(
                "'<' not supported between instances of '{type_name}'",
            )))
        }
    }
}

#[pyclass(module = "jsonexus.indexing.index", name = "RangeIndex")]
pub struct RangeIndex {
    #[pyo3(get)]
    pub field_path: String,
    entries: Vec<RangeEntry>,
    built: bool,
}

impl RangeIndex {
    pub fn new_index(field_path: String) -> Self {
        Self {
            field_path,
            entries: Vec::new(),
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
        let capacity = documents.len().unwrap_or(0);
        let mut entries: Vec<RangeEntry> = Vec::with_capacity(capacity);
        let mut saw_numeric = false;
        let mut saw_text = false;

        for (index, document) in documents.try_iter()?.enumerate() {
            let document = document?;
            let value = match path_get(&document, &self.field_path)? {
                Some(value) => value,
                None => continue,
            };
            if !comparable(value.bind(py)) {
                continue;
            }
            let key = match value_key(value.bind(py), false) {
                Some(key) => key,
                None => continue,
            };
            if key.is_numeric() {
                saw_numeric = true;
            } else {
                saw_text = true;
            }
            let id = document_id_obj(&document)?;
            entries.push(RangeEntry {
                key,
                index,
                id: id.unbind(),
            });
        }

        if saw_numeric && saw_text {
            return Err(PyTypeError::new_err(
                "'<' not supported between instances of 'str' and 'int'",
            ));
        }
        entries.sort_unstable_by(|a, b| a.key.compare(&b.key));

        self.entries = entries;
        self.built = true;
        Ok(())
    }

    fn bisect_left(&self, key: &RangeKey) -> usize {
        let (mut lo, mut hi) = (0usize, self.entries.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if self.entries[mid].key.compare(key) == Ordering::Less {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        lo
    }

    fn bisect_right(&self, key: &RangeKey) -> usize {
        let (mut lo, mut hi) = (0usize, self.entries.len());
        while lo < hi {
            let mid = (lo + hi) / 2;
            if key.compare(&self.entries[mid].key) == Ordering::Less {
                hi = mid;
            } else {
                lo = mid + 1;
            }
        }
        lo
    }

    fn bound_key(
        &self,
        value: &Bound<'_, PyAny>,
        entries_numeric: bool,
    ) -> PyResult<RangeKey> {
        let key = operand_key(value)?;
        if key.is_numeric() != entries_numeric {
            let left = self.entries[0].key.type_name();
            let right = key.type_name();
            return Err(PyTypeError::new_err(format!(
                "'<' not supported between instances of '{left}' and '{right}'",
            )));
        }
        Ok(key)
    }

    /// Positions in the document list matched by the range.
    pub fn range_indices(
        &self,
        low: Option<&Bound<'_, PyAny>>,
        high: Option<&Bound<'_, PyAny>>,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> PyResult<Vec<usize>> {
        if self.entries.is_empty() {
            return Ok(Vec::new());
        }
        let numeric = self.entries[0].key.is_numeric();
        let low_key = match low {
            Some(value) if !value.is_none() => Some(self.bound_key(value, numeric)?),
            _ => None,
        };
        let high_key = match high {
            Some(value) if !value.is_none() => Some(self.bound_key(value, numeric)?),
            _ => None,
        };

        let start = match &low_key {
            Some(key) => {
                if inclusive_low {
                    self.bisect_left(key)
                } else {
                    self.bisect_right(key)
                }
            }
            None => 0,
        };
        let end = match &high_key {
            Some(key) => {
                if inclusive_high {
                    self.bisect_right(key)
                } else {
                    self.bisect_left(key)
                }
            }
            None => self.entries.len(),
        };
        if start >= end {
            return Ok(Vec::new());
        }
        Ok(self.entries[start..end]
            .iter()
            .map(|entry| entry.index)
            .collect())
    }

    pub fn range_objects(
        &self,
        py: Python<'_>,
        low: Option<&Bound<'_, PyAny>>,
        high: Option<&Bound<'_, PyAny>>,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> PyResult<Vec<Py<PyAny>>> {
        let indices = self.range_indices(low, high, inclusive_low, inclusive_high)?;
        Ok(indices
            .into_iter()
            .map(|index| self.entries[index].id.clone_ref(py))
            .collect())
    }
}

#[pymethods]
impl RangeIndex {
    #[new]
    fn new(field_path: String) -> Self {
        Self::new_index(field_path)
    }

    #[getter]
    fn _built(&self) -> bool {
        self.built
    }

    fn rebuild(&mut self, py: Python<'_>, documents: &Bound<'_, PyAny>) -> PyResult<()> {
        self.rebuild_index(py, documents)
    }

    pub fn invalidate_all(&mut self) {
        self.entries.clear();
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
        let ids = self.range_objects(py, low_bound, high_bound, inclusive_low, inclusive_high)?;
        Ok(PyList::new(py, ids)?.unbind())
    }
}
