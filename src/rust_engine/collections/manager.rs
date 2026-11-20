//! Collection implementation.
//!
//! The document list is the *same* Python list object stored in the database's
//! raw dict, so in-place appends/removals are visible through `get_all()` and
//! are what gets persisted.
//!
//! `insert_no_duplicate` intentionally reproduces the destructive legacy
//! behavior: every document it compares has its `_id` deleted first.

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex, MutexGuard};

use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyKeyError, PyRuntimeError, PyTypeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyFloat, PyInt, PyList, PyString};

use crate::indexing::manager::IndexManager;
use crate::query::evaluator::eval_branch;
use crate::query::operators::rich_compare;
use crate::query::parser::normalize;
use crate::utils::id_gen::generate_id;
use crate::utils::validation::{
    validate_document, validate_query, validate_update_fields,
};

pub struct CollectionState {
    pub name: String,
    pub documents: Py<PyList>,
    pub by_id: HashMap<String, usize>,
    pub indexes: Py<IndexManager>,
}

fn py_str(value: &Bound<'_, PyAny>) -> PyResult<String> {
    Ok(value.str()?.to_string_lossy().into_owned())
}

fn document_id(dict: &Bound<'_, PyDict>) -> PyResult<String> {
    match dict.get_item("_id")? {
        Some(id) => py_str(&id),
        None => Err(PyKeyError::new_err("_id")),
    }
}

pub fn rebuild_id_index(documents: &Bound<'_, PyList>) -> PyResult<HashMap<String, usize>> {
    let mut by_id = HashMap::new();
    for (index, document) in documents.iter().enumerate() {
        let dict = document
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
        if dict.contains("_id")? {
            by_id.insert(document_id(dict)?, index);
        }
    }
    Ok(by_id)
}

pub fn state_from_list(
    py: Python<'_>,
    name: String,
    documents: Py<PyList>,
) -> PyResult<CollectionState> {
    let by_id = rebuild_id_index(documents.bind(py))?;
    let indexes = Py::new(py, IndexManager::new_manager())?;
    Ok(CollectionState {
        name,
        documents,
        by_id,
        indexes,
    })
}

fn lock_state(state: &Arc<Mutex<CollectionState>>) -> PyResult<MutexGuard<'_, CollectionState>> {
    state
        .lock()
        .map_err(|_| PyRuntimeError::new_err("collection lock poisoned"))
}

fn ensure_id(py: Python<'_>, document: &Bound<'_, PyAny>) -> PyResult<()> {
    let dict = document
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("Document must be a dict"))?;
    if !dict.contains("_id")? {
        dict.set_item("_id", PyString::new(py, &generate_id()))?;
    }
    Ok(())
}

fn append_document(
    py: Python<'_>,
    state: &mut CollectionState,
    document: &Bound<'_, PyAny>,
) -> PyResult<()> {
    let documents = state.documents.clone_ref(py);
    let index = documents.bind(py).len();
    documents.bind(py).append(document)?;
    let dict = document
        .cast::<PyDict>()
        .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
    state.by_id.insert(document_id(dict)?, index);
    Ok(())
}

fn mark_dirty(py: Python<'_>, state: &CollectionState) {
    state.indexes.borrow(py).mark_dirty(py);
}

fn result_dict(
    py: Python<'_>,
    status: &str,
    data: Option<&Bound<'_, PyAny>>,
) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("status", status)?;
    if let Some(data) = data {
        out.set_item("data", data)?;
    }
    Ok(out.into_any().unbind())
}

fn status_dict(py: Python<'_>, status: &str) -> PyResult<Py<PyAny>> {
    let out = PyDict::new(py);
    out.set_item("status", status)?;
    Ok(out.into_any().unbind())
}

// -- free-function API used by both Collection and JSONexus ------------------

pub fn insert(
    py: Python<'_>,
    state: &mut CollectionState,
    data: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_document(data)?;
    ensure_id(py, data)?;
    append_document(py, state, data)?;
    mark_dirty(py, state);
    result_dict(py, "success", Some(data))
}

pub fn insert_no_duplicate(
    py: Python<'_>,
    state: &mut CollectionState,
    data: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_document(data)?;
    let documents = state.documents.clone_ref(py);
    let length = documents.bind(py).len();

    for index in 0..length {
        let item = documents.bind(py).get_item(index)?;
        let dict = item
            .cast::<PyDict>()
            .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
        // Legacy behavior: strip `_id` from each item as it is checked.
        if dict.contains("_id")? {
            dict.del_item("_id")?;
        }
        if rich_compare(&item, data, CompareOp::Eq)? {
            return result_dict(py, "duplicate exists", Some(&item));
        }
    }

    ensure_id(py, data)?;
    append_document(py, state, data)?;
    mark_dirty(py, state);
    result_dict(py, "success", Some(data))
}

pub fn insert_many(
    py: Python<'_>,
    state: &mut CollectionState,
    data_list: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let list = data_list
        .cast::<PyList>()
        .map_err(|_| PyValueError::new_err("data_list must be a list"))?;
    for item in list.iter() {
        validate_document(&item)?;
        ensure_id(py, &item)?;
        append_document(py, state, &item)?;
    }
    mark_dirty(py, state);
    status_dict(py, "success")
}

fn resolve_ids(
    py: Python<'_>,
    state: &CollectionState,
    ids: impl IntoIterator<Item = String>,
) -> PyResult<Py<PyList>> {
    let documents = state.documents.clone_ref(py);
    let documents = documents.bind(py);
    let mut out: Vec<Py<PyAny>> = Vec::new();
    for id in ids {
        if let Some(&index) = state.by_id.get(&id) {
            out.push(documents.get_item(index)?.unbind());
        }
    }
    Ok(PyList::new(py, out)?.unbind())
}

fn hash_lookup(
    py: Python<'_>,
    state: &mut CollectionState,
    field: &str,
    value: &Bound<'_, PyAny>,
) -> PyResult<Py<PyList>> {
    let documents = state.documents.clone_ref(py);
    let index = state
        .indexes
        .borrow_mut(py)
        .hash_for(py, field, documents.bind(py).as_any())?;
    let ids = index.borrow(py).get_ids(py, value)?;
    resolve_ids(py, state, ids)
}

fn range_or_scan(
    py: Python<'_>,
    state: &mut CollectionState,
    field: &str,
    op: &str,
    operand: &Bound<'_, PyAny>,
) -> PyResult<Option<Py<PyList>>> {
    let (low, high, inclusive_low, inclusive_high) = match op {
        "$gt" => (Some(operand), None, false, true),
        "$gte" => (Some(operand), None, true, true),
        "$lt" => (None, Some(operand), true, false),
        "$lte" => (None, Some(operand), true, true),
        _ => return Ok(None),
    };

    let documents = state.documents.clone_ref(py);
    let index = state.indexes.borrow_mut(py).range_for(
        py,
        field,
        documents.bind(py).as_any(),
    )?;
    let ids = index
        .borrow(py)
        .range_ids(py, low, high, inclusive_low, inclusive_high)?;
    Ok(Some(resolve_ids(py, state, ids)?))
}

fn indexed_find(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyDict>,
) -> PyResult<Option<Py<PyList>>> {
    if query.len() != 1 {
        return Ok(None);
    }
    let (field_obj, value) = match query.iter().next() {
        Some(pair) => pair,
        None => return Ok(None),
    };
    let field: String = match field_obj.extract() {
        Ok(field) => field,
        Err(_) => return Ok(None),
    };

    let is_scalar = value.is_none()
        || value.is_instance_of::<PyString>()
        || value.is_instance_of::<PyInt>()
        || value.is_instance_of::<PyFloat>();

    if is_scalar {
        return Ok(Some(hash_lookup(py, state, &field, &value)?));
    }

    if let Ok(value_dict) = value.cast::<PyDict>() {
        if let Some(operand) = value_dict.get_item("$eq")? {
            return Ok(Some(hash_lookup(py, state, &field, &operand)?));
        }
        if let (true, Some(operand)) = (
            value_dict.contains("_op")?,
            value_dict.get_item("_value")?,
        ) {
            let op_obj = value_dict.get_item("_op")?.expect("checked _op");
            if let Ok(op) = op_obj.extract::<String>() {
                return range_or_scan(py, state, &field, &op, &operand);
            }
            return Ok(None);
        }
        for op in ["$gt", "$gte", "$lt", "$lte"] {
            if let Some(operand) = value_dict.get_item(op)? {
                return range_or_scan(py, state, &field, op, &operand);
            }
        }
    }

    Ok(None)
}

pub fn find(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyAny>,
) -> PyResult<Py<PyList>> {
    validate_query(query)?;
    let query_dict = query.cast::<PyDict>().expect("validated query");
    if let Some(result) = indexed_find(py, state, query_dict)? {
        return Ok(result);
    }

    let nodes = normalize(py, query)?;
    let documents = state.documents.clone_ref(py);
    let mut out: Vec<Py<PyAny>> = Vec::new();
    for document in documents.bind(py).iter() {
        if eval_branch(py, &document, &nodes)? {
            out.push(document.unbind());
        }
    }
    Ok(PyList::new(py, out)?.unbind())
}

pub fn find_one(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyAny>,
) -> PyResult<Option<Py<PyAny>>> {
    let results = find(py, state, query)?;
    Ok(results.bind(py).get_item(0).ok().map(|d| d.unbind()))
}

pub fn count(py: Python<'_>, state: &CollectionState) -> usize {
    state.documents.bind(py).len()
}

pub fn get(
    py: Python<'_>,
    state: &mut CollectionState,
    document_id: &Bound<'_, PyAny>,
) -> PyResult<Option<Py<PyAny>>> {
    let key = py_str(document_id)?;
    let documents = state.documents.clone_ref(py);
    let documents_ref = documents.bind(py);

    let mut index = match state.by_id.get(&key).copied() {
        Some(index) => index,
        None => return Ok(None),
    };

    if index >= documents_ref.len() {
        state.by_id = rebuild_id_index(documents_ref)?;
        match state.by_id.get(&key).copied() {
            Some(found) => index = found,
            None => return Ok(None),
        }
    }

    let document = documents_ref.get_item(index)?;
    let matches = match document.cast::<PyDict>()?.get_item("_id")? {
        Some(id) => py_str(&id)? == key,
        None => "None" == key,
    };
    if matches {
        return Ok(Some(document.unbind()));
    }

    state.by_id = rebuild_id_index(documents_ref)?;
    match state.by_id.get(&key).copied() {
        Some(found) => Ok(Some(documents_ref.get_item(found)?.unbind())),
        None => Ok(None),
    }
}

pub fn update(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyAny>,
    update_fields: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_query(query)?;
    validate_update_fields(update_fields)?;

    let query_dict = query.cast::<PyDict>().expect("validated query");
    let fields = update_fields.cast::<PyDict>().expect("validated fields");

    let mut simple_single = false;
    if query_dict.len() == 1 {
        if let Some((_, only_value)) = query_dict.iter().next() {
            if only_value.is_instance_of::<PyString>() || only_value.is_instance_of::<PyInt>() {
                simple_single = true;
            }
        }
    }

    let matches = find(py, state, query)?;
    for document in matches.bind(py).iter() {
        let dict = document.cast::<PyDict>()?;
        for (key, value) in fields.iter() {
            dict.set_item(key, value)?;
        }
        if simple_single {
            mark_dirty(py, state);
            return status_dict(py, "success");
        }
    }

    mark_dirty(py, state);
    status_dict(py, "success")
}

pub fn update_by_id(
    py: Python<'_>,
    state: &mut CollectionState,
    document_id: &Bound<'_, PyAny>,
    update_fields: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_update_fields(update_fields)?;
    let fields = update_fields.cast::<PyDict>().expect("validated fields");

    if let Some(document) = get(py, state, document_id)? {
        let dict = document.bind(py).cast::<PyDict>()?;
        for (key, value) in fields.iter() {
            dict.set_item(key, value)?;
        }
    }
    mark_dirty(py, state);
    status_dict(py, "success")
}

pub fn delete(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_query(query)?;
    let matches = find(py, state, query)?;

    if matches.bind(py).len() > 0 {
        let mut removed: HashSet<usize> = HashSet::new();
        for document in matches.bind(py).iter() {
            removed.insert(document.as_ptr() as usize);
        }

        let documents = state.documents.clone_ref(py);
        let mut kept: Vec<Py<PyAny>> = Vec::new();
        for document in documents.bind(py).iter() {
            if !removed.contains(&(document.as_ptr() as usize)) {
                kept.push(document.unbind());
            }
        }
        let list = documents.bind(py);
        list.call_method0("clear")?;
        list.call_method1("extend", (PyList::new(py, kept)?,))?;
        state.by_id = rebuild_id_index(list)?;
    }

    mark_dirty(py, state);
    status_dict(py, "success")
}

pub fn delete_by_id(
    py: Python<'_>,
    state: &mut CollectionState,
    document_id: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    let key = py_str(document_id)?;
    let documents = state.documents.clone_ref(py);
    let documents_ref = documents.bind(py);

    if let Some(&index) = state.by_id.get(&key) {
        if index < documents_ref.len() {
            let document = documents_ref.get_item(index)?;
            let same = match document.cast::<PyDict>()?.get_item("_id")? {
                Some(id) => py_str(&id)? == key,
                None => false,
            };
            if same {
                documents_ref.del_item(index)?;
                state.by_id = rebuild_id_index(documents_ref)?;
            }
        }
    }

    mark_dirty(py, state);
    status_dict(py, "success")
}

pub fn match_documents(
    py: Python<'_>,
    state: &mut CollectionState,
    query: &Bound<'_, PyAny>,
) -> PyResult<Py<PyAny>> {
    validate_query(query)?;
    let query_dict = query.cast::<PyDict>().expect("validated query");
    let documents = state.documents.clone_ref(py);

    let mut matched: Vec<Py<PyAny>> = Vec::new();
    for document in documents.bind(py).iter() {
        let dict = document.cast::<PyDict>()?;
        let mut is_match = true;
        for (key, value) in query_dict.iter() {
            match dict.get_item(&key)? {
                Some(field_value) => {
                    if !rich_compare(&field_value, &value, CompareOp::Eq)? {
                        is_match = false;
                        break;
                    }
                }
                None => {
                    is_match = false;
                    break;
                }
            }
        }
        if is_match {
            matched.push(document.unbind());
        }
    }

    let out = PyDict::new(py);
    let result = PyList::new(py, matched)?;
    if result.len() > 0 {
        out.set_item("status", "success")?;
    } else {
        out.set_item("status", "failed")?;
    }
    out.set_item("result", result)?;
    Ok(out.into_any().unbind())
}

pub fn to_list(py: Python<'_>, state: &CollectionState) -> PyResult<Py<PyList>> {
    let documents = state.documents.clone_ref(py);
    Ok(PyList::new(py, documents.bind(py).iter())?.unbind())
}

// -- Python-visible Collection ----------------------------------------------

#[pyclass(module = "jsonexus.collections.manager", name = "Collection")]
pub struct Collection {
    pub state: Arc<Mutex<CollectionState>>,
}

impl Collection {
    pub fn new_state(state: CollectionState) -> Self {
        Self {
            state: Arc::new(Mutex::new(state)),
        }
    }
}

#[pymethods]
impl Collection {
    #[new]
    #[pyo3(signature = (name, documents=None))]
    fn new(
        py: Python<'_>,
        name: String,
        documents: Option<&Bound<'_, PyAny>>,
    ) -> PyResult<Self> {
        let list = PyList::empty(py);
        if let Some(source) = documents {
            for document in source.try_iter()? {
                let document = document?;
                let copy = py
                    .get_type::<PyDict>()
                    .call1((document,))?
                    .cast_into::<PyDict>()
                    .map_err(|_| PyTypeError::new_err("Document must be a dict"))?;
                ensure_id(py, copy.as_any())?;
                list.append(copy)?;
            }
        }
        let state = state_from_list(py, name, list.unbind())?;
        Ok(Self::new_state(state))
    }

    #[getter]
    fn name(&self, _py: Python<'_>) -> PyResult<String> {
        Ok(lock_state(&self.state)?.name.clone())
    }

    #[getter]
    fn indexes(&self, py: Python<'_>) -> PyResult<Py<IndexManager>> {
        Ok(lock_state(&self.state)?.indexes.clone_ref(py))
    }

    fn insert(&self, py: Python<'_>, data: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        insert(py, &mut state, data)
    }

    fn insert_no_duplicate(
        &self,
        py: Python<'_>,
        data: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        insert_no_duplicate(py, &mut state, data)
    }

    fn insert_many(
        &self,
        py: Python<'_>,
        data_list: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        insert_many(py, &mut state, data_list)
    }

    fn find(&self, py: Python<'_>, query: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
        let mut state = lock_state(&self.state)?;
        find(py, &mut state, query)
    }

    fn find_one(
        &self,
        py: Python<'_>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let mut state = lock_state(&self.state)?;
        find_one(py, &mut state, query)
    }

    fn count(&self, py: Python<'_>) -> PyResult<usize> {
        let state = lock_state(&self.state)?;
        Ok(count(py, &state))
    }

    fn get(
        &self,
        py: Python<'_>,
        document_id: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let mut state = lock_state(&self.state)?;
        get(py, &mut state, document_id)
    }

    fn update(
        &self,
        py: Python<'_>,
        query: &Bound<'_, PyAny>,
        update_fields: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        update(py, &mut state, query, update_fields)
    }

    fn update_by_id(
        &self,
        py: Python<'_>,
        document_id: &Bound<'_, PyAny>,
        update_fields: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        update_by_id(py, &mut state, document_id, update_fields)
    }

    fn delete(&self, py: Python<'_>, query: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        delete(py, &mut state, query)
    }

    fn delete_by_id(
        &self,
        py: Python<'_>,
        document_id: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        delete_by_id(py, &mut state, document_id)
    }

    #[pyo3(name = "match")]
    fn match_(&self, py: Python<'_>, query: &Bound<'_, PyAny>) -> PyResult<Py<PyAny>> {
        let mut state = lock_state(&self.state)?;
        match_documents(py, &mut state, query)
    }

    fn to_list(&self, py: Python<'_>) -> PyResult<Py<PyList>> {
        let state = lock_state(&self.state)?;
        to_list(py, &state)
    }

    fn __len__(&self, py: Python<'_>) -> PyResult<usize> {
        let state = lock_state(&self.state)?;
        Ok(state.documents.bind(py).len())
    }

    fn __contains__(&self, document_id: &Bound<'_, PyAny>) -> PyResult<bool> {
        let state = lock_state(&self.state)?;
        Ok(state.by_id.contains_key(&py_str(document_id)?))
    }

    fn __iter__(&self, py: Python<'_>) -> PyResult<Py<PyAny>> {
        let state = lock_state(&self.state)?;
        let documents = state.documents.clone_ref(py);
        Ok(documents.bind(py).try_iter()?.unbind().into_any())
    }
}
