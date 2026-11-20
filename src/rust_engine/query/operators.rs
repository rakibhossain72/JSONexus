//! Query operators.
//!
//! Each operator mirrors the pure-Python implementation, including which
//! exceptions are swallowed (`TypeError` for comparisons, `$mod` also
//! swallowing `ZeroDivisionError`/`ValueError`) and which are propagated.
//! All comparisons use Python rich comparison so mixed types behave exactly
//! like the original.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use pyo3::basic::CompareOp;
use pyo3::exceptions::{PyTypeError, PyValueError, PyZeroDivisionError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyInt, PyList, PyString};

pub fn rich_compare(
    a: &Bound<'_, PyAny>,
    b: &Bound<'_, PyAny>,
    op: CompareOp,
) -> PyResult<bool> {
    a.rich_compare(b, op)?.is_truthy()
}

fn is_type_error(py: Python<'_>, err: &PyErr) -> bool {
    err.is_instance_of::<PyTypeError>(py)
}

fn swallow_type_error(py: Python<'_>, result: PyResult<bool>) -> PyResult<bool> {
    match result {
        Ok(value) => Ok(value),
        Err(err) if is_type_error(py, &err) => Ok(false),
        Err(err) => Err(err),
    }
}

/// Evaluate `op` against a present field value.
pub fn apply(
    py: Python<'_>,
    op: &str,
    field: &Bound<'_, PyAny>,
    operand: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    match op {
        "$eq" => rich_compare(field, operand, CompareOp::Eq),
        "$ne" => rich_compare(field, operand, CompareOp::Ne),
        "$gt" => swallow_type_error(py, rich_compare(field, operand, CompareOp::Gt)),
        "$gte" => swallow_type_error(py, rich_compare(field, operand, CompareOp::Ge)),
        "$lt" => swallow_type_error(py, rich_compare(field, operand, CompareOp::Lt)),
        "$lte" => swallow_type_error(py, rich_compare(field, operand, CompareOp::Le)),
        "$in" => swallow_type_error(py, operand.contains(field)),
        "$nin" => match operand.contains(field) {
            Ok(found) => Ok(!found),
            Err(err) if is_type_error(py, &err) => Ok(false),
            Err(err) => Err(err),
        },
        // `field` is only ever absent when the evaluator special-cases
        // `$exists`; by the time this is called the value is present.
        "$exists" => Ok(operand.is_truthy()?),
        "$regex" => regex(field, operand),
        "$size" => size(field, operand),
        "$all" => all_operator(py, field, operand),
        "$mod" => modulo(py, field, operand),
        "$elemMatch" => elem_match(py, field, operand),
        _ => Err(PyValueError::new_err(format!("Unknown operator: {op}"))),
    }
}

fn regex(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    if field.cast::<PyString>().is_err() || operand.cast::<PyString>().is_err() {
        return Ok(false);
    }
    let pattern: String = operand.extract()?;
    let compiled = compiled_regex(field.py(), &pattern)?;
    let found = compiled
        .bind(field.py())
        .call_method1("search", (field,))?;
    Ok(!found.is_none())
}

fn compiled_regex(py: Python<'_>, pattern: &str) -> PyResult<Py<PyAny>> {
    static CACHE: OnceLock<Mutex<HashMap<String, Py<PyAny>>>> = OnceLock::new();
    let cache = CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    if let Some(hit) = cache.lock().unwrap().get(pattern) {
        return Ok(hit.clone_ref(py));
    }
    let compiled = py
        .import("re")?
        .getattr("compile")?
        .call1((pattern,))?
        .unbind();
    cache
        .lock()
        .unwrap()
        .insert(pattern.to_string(), compiled.clone_ref(py));
    Ok(compiled)
}

fn size(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    let py = field.py();
    match field.len() {
        Ok(len) => {
            let len_obj = PyInt::new(py, len as u64);
            rich_compare(len_obj.as_any(), operand, CompareOp::Eq)
        }
        Err(err) if is_type_error(py, &err) => Ok(false),
        Err(err) => Err(err),
    }
}

fn all_operator(
    py: Python<'_>,
    field: &Bound<'_, PyAny>,
    operand: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let iter = match operand.try_iter() {
        Ok(iter) => iter,
        Err(err) if is_type_error(py, &err) => return Ok(false),
        Err(err) => return Err(err),
    };
    for item in iter {
        let item = match item {
            Ok(item) => item,
            Err(err) if is_type_error(py, &err) => return Ok(false),
            Err(err) => return Err(err),
        };
        match field.contains(&item) {
            Ok(true) => {}
            Ok(false) => return Ok(false),
            Err(err) if is_type_error(py, &err) => return Ok(false),
            Err(err) => return Err(err),
        }
    }
    Ok(true)
}

fn swallow_modulo_error(py: Python<'_>, err: &PyErr) -> bool {
    err.is_instance_of::<PyTypeError>(py)
        || err.is_instance_of::<PyZeroDivisionError>(py)
        || err.is_instance_of::<PyValueError>(py)
}

fn modulo(
    py: Python<'_>,
    field: &Bound<'_, PyAny>,
    operand: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let mut iter = match operand.try_iter() {
        Ok(iter) => iter,
        Err(err) if is_type_error(py, &err) => return Ok(false),
        Err(err) => return Err(err),
    };
    let mut values: Vec<Bound<'_, PyAny>> = Vec::new();
    while values.len() < 3 {
        match iter.next() {
            Some(Ok(value)) => values.push(value),
            Some(Err(err)) if is_type_error(py, &err) => return Ok(false),
            Some(Err(err)) => return Err(err),
            None => break,
        }
    }
    if values.len() != 2 {
        return Ok(false);
    }
    let operator = py.import("operator")?;
    let remainder = match operator
        .getattr("mod")?
        .call1((field, values[0].clone()))
    {
        Ok(value) => value,
        Err(err) if swallow_modulo_error(py, &err) => return Ok(false),
        Err(err) => return Err(err),
    };
    rich_compare(&remainder, &values[1], CompareOp::Eq)
}

fn elem_match(
    py: Python<'_>,
    field: &Bound<'_, PyAny>,
    operand: &Bound<'_, PyAny>,
) -> PyResult<bool> {
    let list = match field.cast::<PyList>() {
        Ok(list) => list,
        Err(_) => return Ok(false),
    };
    if operand.cast::<PyDict>().is_err() {
        return Ok(false);
    }
    let nodes = crate::query::parser::normalize(py, operand)?;
    for item in list.iter() {
        if item.cast::<PyDict>().is_ok()
            && crate::query::evaluator::eval_branch(py, &item, &nodes)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

// -- Python-visible operator callables (used by the operators shim) ----------

#[pyfunction]
pub fn op_eq(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$eq", field, operand)
}

#[pyfunction]
pub fn op_ne(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$ne", field, operand)
}

#[pyfunction]
pub fn op_gt(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$gt", field, operand)
}

#[pyfunction]
pub fn op_gte(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$gte", field, operand)
}

#[pyfunction]
pub fn op_lt(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$lt", field, operand)
}

#[pyfunction]
pub fn op_lte(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$lte", field, operand)
}

#[pyfunction]
pub fn op_in(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$in", field, operand)
}

#[pyfunction]
pub fn op_nin(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$nin", field, operand)
}

#[pyfunction]
pub fn op_exists(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$exists", field, operand)
}

#[pyfunction]
pub fn op_regex(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$regex", field, operand)
}

#[pyfunction]
pub fn op_size(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$size", field, operand)
}

#[pyfunction]
pub fn op_all(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$all", field, operand)
}

#[pyfunction]
pub fn op_mod(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$mod", field, operand)
}

#[pyfunction]
pub fn op_elem_match(field: &Bound<'_, PyAny>, operand: &Bound<'_, PyAny>) -> PyResult<bool> {
    apply(field.py(), "$elemMatch", field, operand)
}
