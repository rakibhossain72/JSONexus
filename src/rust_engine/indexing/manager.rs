//! Per-collection index registry.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyList, PySet};
use pyo3::types::PyString;

use crate::indexing::index::{HashIndex, RangeIndex};

#[pyclass(module = "jsonexus.indexing.manager", name = "IndexManager")]
pub struct IndexManager {
    hash: HashMap<String, Py<HashIndex>>,
    range: HashMap<String, Py<RangeIndex>>,
}

impl IndexManager {
    pub fn new_manager() -> Self {
        Self {
            hash: HashMap::new(),
            range: HashMap::new(),
        }
    }

    /// Return the hash index for `field_path`, building it if stale.
    pub fn hash_for(
        &mut self,
        py: Python<'_>,
        field_path: &str,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<Py<HashIndex>> {
        if !self.hash.contains_key(field_path) {
            self.hash.insert(
                field_path.to_string(),
                Py::new(py, HashIndex::new_index(py, field_path.to_string()))?,
            );
        }
        let index = self.hash.get(field_path).unwrap().clone_ref(py);
        if !index.borrow(py).is_built() {
            index.borrow_mut(py).rebuild_index(py, documents)?;
        }
        Ok(index)
    }

    /// Return the range index for `field_path`, building it if stale.
    pub fn range_for(
        &mut self,
        py: Python<'_>,
        field_path: &str,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<Py<RangeIndex>> {
        if !self.range.contains_key(field_path) {
            self.range.insert(
                field_path.to_string(),
                Py::new(py, RangeIndex::new_index(py, field_path.to_string()))?,
            );
        }
        let index = self.range.get(field_path).unwrap().clone_ref(py);
        if !index.borrow(py).is_built() {
            index.borrow_mut(py).rebuild_index(py, documents)?;
        }
        Ok(index)
    }
}

#[pymethods]
impl IndexManager {
    #[new]
    fn new() -> Self {
        Self::new_manager()
    }

    /// Invalidate every index (call after any mutation).
    pub fn mark_dirty(&self, py: Python<'_>) {
        for index in self.hash.values() {
            index.borrow_mut(py).invalidate_all(py);
        }
        for index in self.range.values() {
            index.borrow_mut(py).invalidate_all(py);
        }
    }

    fn reset(&mut self) {
        self.hash.clear();
        self.range.clear();
    }

    fn rebuild_all(&self, py: Python<'_>, documents: &Bound<'_, PyAny>) -> PyResult<()> {
        for index in self.hash.values() {
            index.borrow_mut(py).rebuild_index(py, documents)?;
        }
        for index in self.range.values() {
            index.borrow_mut(py).rebuild_index(py, documents)?;
        }
        Ok(())
    }

    fn ensure_hash(&mut self, py: Python<'_>, field_path: &str) -> PyResult<Py<HashIndex>> {
        if !self.hash.contains_key(field_path) {
            self.hash.insert(
                field_path.to_string(),
                Py::new(py, HashIndex::new_index(py, field_path.to_string()))?,
            );
        }
        Ok(self.hash.get(field_path).unwrap().clone_ref(py))
    }

    fn ensure_range(&mut self, py: Python<'_>, field_path: &str) -> PyResult<Py<RangeIndex>> {
        if !self.range.contains_key(field_path) {
            self.range.insert(
                field_path.to_string(),
                Py::new(py, RangeIndex::new_index(py, field_path.to_string()))?,
            );
        }
        Ok(self.range.get(field_path).unwrap().clone_ref(py))
    }

    fn get_or_build_hash(
        &mut self,
        py: Python<'_>,
        field_path: &str,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<Py<HashIndex>> {
        self.hash_for(py, field_path, documents)
    }

    fn get_or_build_range(
        &mut self,
        py: Python<'_>,
        field_path: &str,
        documents: &Bound<'_, PyAny>,
    ) -> PyResult<Py<RangeIndex>> {
        self.range_for(py, field_path, documents)
    }

    fn try_equality(
        &self,
        py: Python<'_>,
        field_path: &str,
        value: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PySet>>> {
        match self.hash.get(field_path) {
            Some(index) if index.borrow(py).is_built() => {
                let ids = index.borrow(py).get_ids(py, value)?;
                let set = PySet::empty(py)?;
                for id in ids {
                    set.add(id)?;
                }
                Ok(Some(set.unbind()))
            }
            _ => Ok(None),
        }
    }

    fn try_range(
        &self,
        py: Python<'_>,
        field_path: &str,
        low: Option<Py<PyAny>>,
        high: Option<Py<PyAny>>,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> PyResult<Option<Py<PyList>>> {
        match self.range.get(field_path) {
            Some(index) if index.borrow(py).is_built() => {
                let low_bound = low.as_ref().map(|v| v.bind(py));
                let high_bound = high.as_ref().map(|v| v.bind(py));
                let ids = index.borrow(py).range_ids(
                    py,
                    low_bound,
                    high_bound,
                    inclusive_low,
                    inclusive_high,
                )?;
                let out: Vec<Py<PyAny>> = ids
                    .iter()
                    .map(|id| PyString::new(py, id).into_any().unbind())
                    .collect();
                Ok(Some(PyList::new(py, out)?.unbind()))
            }
            _ => Ok(None),
        }
    }
}
