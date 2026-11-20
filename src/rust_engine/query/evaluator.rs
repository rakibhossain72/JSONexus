//! Query evaluation over live Python document dicts.

use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList};

use crate::query::parser::{nodes_from_py, Condition, Logical, Node};

/// Fetch a possibly-dotted path. `None` is the MISSING sentinel.
pub fn path_get(document: &Bound<'_, PyAny>, path: &str) -> PyResult<Option<Py<PyAny>>> {
    if path.is_empty() {
        return Ok(Some(document.clone().unbind()));
    }
    let mut current: Bound<'_, PyAny> = document.clone();
    for part in path.split('.') {
        let next = {
            let dict = match current.cast::<PyDict>() {
                Ok(dict) => dict,
                Err(_) => return Ok(None),
            };
            dict.get_item(part)?
        };
        match next {
            Some(value) => current = value,
            None => return Ok(None),
        }
    }
    Ok(Some(current.unbind()))
}

pub fn eval_branch(
    py: Python<'_>,
    document: &Bound<'_, PyAny>,
    nodes: &[Node],
) -> PyResult<bool> {
    for node in nodes {
        let matches = match node {
            Node::Leaf(condition) => eval_leaf(py, document, condition)?,
            Node::Logical(logical) => eval_logical(py, document, logical)?,
        };
        if !matches {
            return Ok(false);
        }
    }
    Ok(true)
}

fn eval_logical(
    py: Python<'_>,
    document: &Bound<'_, PyAny>,
    logical: &Logical,
) -> PyResult<bool> {
    match logical.op.as_str() {
        "$and" => {
            for branch in &logical.branches {
                if !eval_branch(py, document, branch)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        "$or" => {
            for branch in &logical.branches {
                if eval_branch(py, document, branch)? {
                    return Ok(true);
                }
            }
            Ok(false)
        }
        "$nor" => {
            for branch in &logical.branches {
                if eval_branch(py, document, branch)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}

fn eval_leaf(
    py: Python<'_>,
    document: &Bound<'_, PyAny>,
    condition: &Condition,
) -> PyResult<bool> {
    let operand = condition.operand.bind(py);

    if condition.operator == "$exists" {
        let present = path_get(document, &condition.field_path)?.is_some();
        return Ok(present == operand.is_truthy()?);
    }

    let value = match path_get(document, &condition.field_path)? {
        Some(value) => value,
        // Missing fields never match except `$ne`.
        None => return Ok(condition.operator == "$ne"),
    };

    crate::query::operators::apply(py, &condition.operator, value.bind(py), operand)
}

/// Stateless evaluator, mirroring `jsonexus.query.evaluator.QueryEvaluator`.
#[pyclass(module = "jsonexus.query.evaluator", name = "QueryEvaluator")]
pub struct QueryEvaluator;

#[pymethods]
impl QueryEvaluator {
    #[new]
    fn new() -> Self {
        Self
    }

    fn matches(
        &self,
        py: Python<'_>,
        document: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<bool> {
        let nodes = nodes_from_py(py, query)?;
        eval_branch(py, document, &nodes)
    }

    fn find_all(
        &self,
        py: Python<'_>,
        documents: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Py<PyList>> {
        let nodes = nodes_from_py(py, query)?;
        let mut out: Vec<Py<PyAny>> = Vec::new();
        for document in documents.try_iter()? {
            let document = document?;
            if eval_branch(py, &document, &nodes)? {
                out.push(document.unbind());
            }
        }
        Ok(PyList::new(py, out)?.unbind())
    }

    fn find_one(
        &self,
        py: Python<'_>,
        documents: &Bound<'_, PyAny>,
        query: &Bound<'_, PyAny>,
    ) -> PyResult<Option<Py<PyAny>>> {
        let nodes = nodes_from_py(py, query)?;
        for document in documents.try_iter()? {
            let document = document?;
            if eval_branch(py, &document, &nodes)? {
                return Ok(Some(document.unbind()));
            }
        }
        Ok(None)
    }
}
