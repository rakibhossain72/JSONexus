//! Per-collection index registry.

use std::collections::HashMap;

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PySet};

use crate::indexing::index::{HashIndex, RangeIndex};
use crate::query::evaluator::path_get;

/// A built hash index's state for one document before an in-place update.
pub struct HashSnapshot {
    pub field: String,
    pub value: Option<Py<PyAny>>,
    pub id: Py<PyAny>,
}

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
                Py::new(py, RangeIndex::new_index(field_path.to_string()))?,
            );
        }
        let index = self.range.get(field_path).unwrap().clone_ref(py);
        if !index.borrow(py).is_built() {
            index.borrow_mut(py).rebuild_index(py, documents)?;
        }
        Ok(index)
    }

    /// Snapshot the values of every registered hash field for one document.
    ///
    /// Only a handful of fields are ever indexed, so this is much cheaper than
    /// copying whole documents before an update.
    pub fn snapshot_hash(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyAny>,
    ) -> PyResult<Vec<HashSnapshot>> {
        if self.hash.is_empty() {
            return Ok(Vec::new());
        }
        let dict = match document.cast::<PyDict>() {
            Ok(dict) => dict,
            Err(_) => return Ok(Vec::new()),
        };
        let id = match dict.get_item("_id")? {
            Some(id) => id.unbind(),
            None => return Ok(Vec::new()),
        };
        let mut snapshots = Vec::with_capacity(self.hash.len());
        for field in self.hash.keys() {
            snapshots.push(HashSnapshot {
                field: field.clone(),
                value: path_get(document, field)?,
                id: id.clone_ref(py),
            });
        }
        Ok(snapshots)
    }

    /// Add a freshly inserted document to every *built* hash index.
    pub fn on_insert(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        for index in self.hash.values() {
            if index.borrow(py).is_built() {
                index.borrow_mut(py).add(py, document)?;
            }
        }
        Ok(())
    }

    /// Remove a deleted document from every *built* hash index.
    pub fn on_remove(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyAny>,
    ) -> PyResult<()> {
        for index in self.hash.values() {
            if index.borrow(py).is_built() {
                index.borrow_mut(py).remove(py, document)?;
            }
        }
        Ok(())
    }

    /// Apply an in-place update to every *built* hash index.
    pub fn on_update(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyAny>,
        snapshots: &[HashSnapshot],
    ) -> PyResult<()> {
        if snapshots.is_empty() {
            return Ok(());
        }
        let new_id = match document.cast::<PyDict>()?.get_item("_id")? {
            Some(id) => id,
            None => return Ok(()),
        };
        for snapshot in snapshots {
            if let Some(index) = self.hash.get(&snapshot.field) {
                if index.borrow(py).is_built() {
                    let new_value = path_get(document, &snapshot.field)?;
                    index.borrow_mut(py).update_values(
                        py,
                        snapshot.value.as_ref().map(|value| value.bind(py)),
                        new_value.as_ref().map(|value| value.bind(py)),
                        snapshot.id.bind(py),
                        &new_id,
                        document,
                    )?;
                }
            }
        }
        Ok(())
    }

    /// Invalidate only range indexes; hash indexes stay incrementally valid.
    pub fn mark_range_dirty(&self, py: Python<'_>) {
        for index in self.range.values() {
            index.borrow_mut(py).invalidate_all();
        }
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
            index.borrow_mut(py).invalidate_all();
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
                Py::new(py, RangeIndex::new_index(field_path.to_string()))?,
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
                Ok(Some(index.borrow(py).get(py, value)?))
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
                let ids = index.borrow(py).range_objects(
                    py,
                    low_bound,
                    high_bound,
                    inclusive_low,
                    inclusive_high,
                )?;
                Ok(Some(PyList::new(py, ids)?.unbind()))
            }
            _ => Ok(None),
        }
    }
}
