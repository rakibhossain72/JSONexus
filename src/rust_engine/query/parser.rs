//! Query normalization.
//!
//! Produces the same `Condition`/`Logical` tree as the pure-Python parser
//! (legacy `{_op, _value}`, canonical `{$op: v}`, logical combinators, and
//! dotted paths), while also exposing Python-visible node objects for
//! compatibility with `jsonexus.query.parser`.

use pyo3::exceptions::{PyAttributeError, PyValueError};
use pyo3::prelude::*;
use pyo3::types::{PyDict, PyList, PyString};

pub const KNOWN_OPERATORS: &[&str] = &[
    "$eq",
    "$ne",
    "$gt",
    "$gte",
    "$lt",
    "$lte",
    "$in",
    "$nin",
    "$exists",
    "$regex",
    "$size",
    "$all",
    "$mod",
    "$elemMatch",
];

pub struct Condition {
    pub field_path: String,
    pub operator: String,
    pub operand: Py<PyAny>,
}

pub struct Logical {
    pub op: String,
    pub branches: Vec<Vec<Node>>,
}

pub enum Node {
    Leaf(Condition),
    Logical(Logical),
}

fn non_string_key_error(key: &Bound<'_, PyAny>) -> PyErr {
    let type_name = key
        .get_type()
        .name()
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "object".to_string());
    PyAttributeError::new_err(format!("'{}' object has no attribute 'split'", type_name))
}

fn normalize_leaf(
    field: &str,
    field_obj: &Bound<'_, PyAny>,
    value: &Bound<'_, PyAny>,
) -> PyResult<Condition> {
    if let Ok(value_dict) = value.cast::<PyDict>() {
        if value_dict.contains("_op")? && value_dict.contains("_value")? {
            let op_obj = value_dict
                .get_item("_op")?
                .expect("checked _op above");
            let operator = match op_obj.extract::<String>() {
                Ok(op) => op,
                Err(_) => {
                    let repr = op_obj.repr()?.to_string();
                    return Err(PyValueError::new_err(format!("Unknown operator: {repr}")));
                }
            };
            let operand = value_dict
                .get_item("_value")?
                .expect("checked _value above")
                .unbind();
            return Ok(Condition {
                field_path: field.to_string(),
                operator,
                operand,
            });
        }

        for (op_key, operand) in value_dict.iter() {
            if let Ok(op) = op_key.extract::<String>() {
                if KNOWN_OPERATORS.contains(&op.as_str()) {
                    return Ok(Condition {
                        field_path: field.to_string(),
                        operator: op,
                        operand: operand.unbind(),
                    });
                }
            }
        }

        let field_repr = field_obj.repr()?.to_string();
        let value_repr = value.repr()?.to_string();
        return Err(PyValueError::new_err(format!(
            "Unknown operator shape for field {field_repr}: {value_repr}"
        )));
    }

    Ok(Condition {
        field_path: field.to_string(),
        operator: "$eq".to_string(),
        operand: value.clone().unbind(),
    })
}

/// Parse a raw query dict into a normalized node list.
pub fn normalize(py: Python<'_>, query: &Bound<'_, PyAny>) -> PyResult<Vec<Node>> {
    let _ = py;
    let query_dict = query
        .cast::<PyDict>()
        .map_err(|_| PyValueError::new_err("Query must be a dict"))?;

    let mut result = Vec::new();
    for (key_obj, value) in query_dict.iter() {
        let key: String = key_obj
            .extract()
            .map_err(|_| non_string_key_error(&key_obj))?;

        if key == "$and" || key == "$or" || key == "$nor" {
            let list = value.cast::<PyList>().map_err(|_| {
                PyValueError::new_err(format!("{key} expects a list operand"))
            })?;
            let mut branches = Vec::new();
            for branch in list.iter() {
                branches.push(normalize(py, &branch)?);
            }
            result.push(Node::Logical(Logical {
                op: key,
                branches,
            }));
        } else {
            result.push(Node::Leaf(normalize_leaf(&key, &key_obj, &value)?));
        }
    }
    Ok(result)
}

// -- Python-visible node classes --------------------------------------------

#[pyclass(frozen, module = "jsonexus.query.parser", name = "Condition")]
pub struct PyCondition {
    #[pyo3(get)]
    pub field_path: String,
    #[pyo3(get)]
    pub operator: String,
    #[pyo3(get)]
    pub operand: Py<PyAny>,
}

#[pymethods]
impl PyCondition {
    #[new]
    #[pyo3(signature = (field_path, operator, operand))]
    fn new(field_path: String, operator: String, operand: Py<PyAny>) -> Self {
        Self {
            field_path,
            operator,
            operand,
        }
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let field_path = PyString::new(py, &self.field_path).repr()?.to_string();
        let operator = PyString::new(py, &self.operator).repr()?.to_string();
        let operand = self.operand.bind(py).repr()?.to_string();
        Ok(format!(
            "Condition(field_path={field_path}, operator={operator}, operand={operand})"
        ))
    }
}

#[pyclass(frozen, module = "jsonexus.query.parser", name = "Logical")]
pub struct PyLogical {
    #[pyo3(get)]
    pub op: String,
    #[pyo3(get)]
    pub conditions: Py<PyAny>,
}

#[pymethods]
impl PyLogical {
    #[new]
    #[pyo3(signature = (op, conditions))]
    fn new(op: String, conditions: Py<PyAny>) -> Self {
        Self { op, conditions }
    }

    fn __repr__(&self, py: Python<'_>) -> PyResult<String> {
        let op = PyString::new(py, &self.op).repr()?.to_string();
        let conditions = self.conditions.bind(py).repr()?.to_string();
        Ok(format!("Logical(op={op}, conditions={conditions})"))
    }
}

fn nodes_to_py(py: Python<'_>, nodes: &[Node]) -> PyResult<Py<PyList>> {
    let mut objects: Vec<Py<PyAny>> = Vec::new();
    for node in nodes {
        match node {
            Node::Leaf(cond) => {
                let obj = Py::new(
                    py,
                    PyCondition {
                        field_path: cond.field_path.clone(),
                        operator: cond.operator.clone(),
                        operand: cond.operand.clone_ref(py),
                    },
                )?;
                objects.push(obj.into_any());
            }
            Node::Logical(logical) => {
                let mut branches: Vec<Py<PyAny>> = Vec::new();
                for branch in &logical.branches {
                    branches.push(nodes_to_py(py, branch)?.into_any());
                }
                let conditions = PyList::new(py, branches)?.unbind().into_any();
                let obj = Py::new(
                    py,
                    PyLogical {
                        op: logical.op.clone(),
                        conditions,
                    },
                )?;
                objects.push(obj.into_any());
            }
        }
    }
    Ok(PyList::new(py, objects)?.unbind())
}

#[pyfunction(name = "normalize")]
pub fn normalize_query(py: Python<'_>, query: &Bound<'_, PyAny>) -> PyResult<Py<PyList>> {
    let nodes = normalize(py, query)?;
    nodes_to_py(py, &nodes)
}

#[pyfunction]
pub fn is_leaf(node: &Bound<'_, PyAny>) -> bool {
    node.cast::<PyCondition>().is_ok()
}

#[pyfunction]
pub fn is_logical(node: &Bound<'_, PyAny>) -> bool {
    node.cast::<PyLogical>().is_ok()
}

/// Convert a node list (or a raw query dict) from Python into Rust nodes.
pub fn nodes_from_py(py: Python<'_>, source: &Bound<'_, PyAny>) -> PyResult<Vec<Node>> {
    if source.cast::<PyDict>().is_ok() {
        return normalize(py, source);
    }
    let mut nodes = Vec::new();
    let iter = source.try_iter()?;
    for item in iter {
        nodes.push(node_from_py(py, &item?)?);
    }
    Ok(nodes)
}

fn node_from_py(py: Python<'_>, obj: &Bound<'_, PyAny>) -> PyResult<Node> {
    if let Ok(cond) = obj.cast::<PyCondition>() {
        let cond = cond.borrow();
        return Ok(Node::Leaf(Condition {
            field_path: cond.field_path.clone(),
            operator: cond.operator.clone(),
            operand: cond.operand.clone_ref(py),
        }));
    }
    if let Ok(logical) = obj.cast::<PyLogical>() {
        let logical = logical.borrow();
        let conditions = logical.conditions.bind(py);
        let mut branches = Vec::new();
        for branch in conditions.try_iter()? {
            let branch = branch?;
            let mut nodes = Vec::new();
            for node in branch.try_iter()? {
                nodes.push(node_from_py(py, &node?)?);
            }
            branches.push(nodes);
        }
        return Ok(Node::Logical(Logical {
            op: logical.op.clone(),
            branches,
        }));
    }
    let type_name = obj
        .get_type()
        .name()
        .map(|n| n.to_string())
        .unwrap_or_else(|_| "object".to_string());
    Err(PyAttributeError::new_err(format!(
        "'{}' object has no attribute 'op'",
        type_name
    )))
}
