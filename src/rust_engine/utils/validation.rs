//! Input validation helpers.
//!
//! Every helper raises the same `ValueError` messages as
//! `jsonexus/utils/validation.py`.

use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyString};

pub fn validate_collection_name(name: &Bound<'_, PyAny>) -> PyResult<()> {
    if name.cast::<PyString>().is_ok() && name.len()? > 0 {
        Ok(())
    } else {
        Err(PyValueError::new_err(
            "Collection name must be a non-empty string",
        ))
    }
}

pub fn validate_document(doc: &Bound<'_, PyAny>) -> PyResult<()> {
    if doc.cast::<PyDict>().is_ok() {
        Ok(())
    } else {
        Err(PyValueError::new_err("Document must be a dict"))
    }
}

pub fn validate_query(query: &Bound<'_, PyAny>) -> PyResult<()> {
    if query.cast::<PyDict>().is_ok() {
        Ok(())
    } else {
        Err(PyValueError::new_err("Query must be a dict"))
    }
}

pub fn validate_update_fields(fields: &Bound<'_, PyAny>) -> PyResult<()> {
    if fields.cast::<PyDict>().is_ok() {
        Ok(())
    } else {
        Err(PyValueError::new_err("Update fields must be a dict"))
    }
}

#[pyfunction(name = "validate_collection_name")]
pub fn validate_collection_name_py(name: &Bound<'_, PyAny>) -> PyResult<()> {
    validate_collection_name(name)
}

#[pyfunction(name = "validate_document")]
pub fn validate_document_py(doc: &Bound<'_, PyAny>) -> PyResult<()> {
    validate_document(doc)
}

#[pyfunction(name = "validate_query")]
pub fn validate_query_py(query: &Bound<'_, PyAny>) -> PyResult<()> {
    validate_query(query)
}

#[pyfunction(name = "validate_update_fields")]
pub fn validate_update_fields_py(fields: &Bound<'_, PyAny>) -> PyResult<()> {
    validate_update_fields(fields)
}
