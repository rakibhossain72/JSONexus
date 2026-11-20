//! PyO3 entry point for the JSONexus Rust engine.

use pyo3::prelude::*;

mod collections;
mod core;
mod indexing;
mod query;
mod storage;
mod utils;

use collections::manager::Collection;
use core::database::JSONexus;
use indexing::index::{HashIndex, RangeIndex};
use indexing::manager::IndexManager;
use query::evaluator::QueryEvaluator;
use query::operators::*;
use query::parser::{
    is_leaf, is_logical, normalize_query, PyCondition, PyLogical,
};
use storage::backend::StorageBackendBase;
use storage::file_backend::FileStorageBackend;
use utils::id_gen::{generate_id_py, short_id_py};
use utils::validation::{
    validate_collection_name_py, validate_document_py, validate_query_py,
    validate_update_fields_py,
};

#[pymodule]
fn _rust_engine(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<JSONexus>()?;
    m.add_class::<Collection>()?;
    m.add_class::<IndexManager>()?;
    m.add_class::<HashIndex>()?;
    m.add_class::<RangeIndex>()?;
    m.add_class::<QueryEvaluator>()?;
    m.add_class::<PyCondition>()?;
    m.add_class::<PyLogical>()?;
    m.add_class::<StorageBackendBase>()?;
    m.add_class::<FileStorageBackend>()?;

    m.add_function(wrap_pyfunction!(normalize_query, m)?)?;
    m.add_function(wrap_pyfunction!(is_leaf, m)?)?;
    m.add_function(wrap_pyfunction!(is_logical, m)?)?;
    m.add_function(wrap_pyfunction!(generate_id_py, m)?)?;
    m.add_function(wrap_pyfunction!(short_id_py, m)?)?;
    m.add_function(wrap_pyfunction!(validate_collection_name_py, m)?)?;
    m.add_function(wrap_pyfunction!(validate_document_py, m)?)?;
    m.add_function(wrap_pyfunction!(validate_query_py, m)?)?;
    m.add_function(wrap_pyfunction!(validate_update_fields_py, m)?)?;

    m.add_function(wrap_pyfunction!(op_eq, m)?)?;
    m.add_function(wrap_pyfunction!(op_ne, m)?)?;
    m.add_function(wrap_pyfunction!(op_gt, m)?)?;
    m.add_function(wrap_pyfunction!(op_gte, m)?)?;
    m.add_function(wrap_pyfunction!(op_lt, m)?)?;
    m.add_function(wrap_pyfunction!(op_lte, m)?)?;
    m.add_function(wrap_pyfunction!(op_in, m)?)?;
    m.add_function(wrap_pyfunction!(op_nin, m)?)?;
    m.add_function(wrap_pyfunction!(op_exists, m)?)?;
    m.add_function(wrap_pyfunction!(op_regex, m)?)?;
    m.add_function(wrap_pyfunction!(op_size, m)?)?;
    m.add_function(wrap_pyfunction!(op_all, m)?)?;
    m.add_function(wrap_pyfunction!(op_mod, m)?)?;
    m.add_function(wrap_pyfunction!(op_elem_match, m)?)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use pyo3::types::{PyDict, PyList};

    #[test]
    fn id_generation_matches_python_shapes() {
        let id = utils::id_gen::generate_id();
        assert_eq!(id.len(), 36);
        assert_eq!(id.matches('-').count(), 4);
        assert_eq!(utils::id_gen::short_id().len(), 8);
    }

    #[test]
    fn normalizes_legacy_and_canonical_queries() {
        Python::initialize();
        Python::attach(|py| {
            let document = PyDict::new(py);
            document.set_item("age", 30).unwrap();
            document.set_item("name", "Alice").unwrap();

            let legacy = PyDict::new(py);
            let condition = PyDict::new(py);
            condition.set_item("_op", "$gte").unwrap();
            condition.set_item("_value", 25).unwrap();
            legacy.set_item("age", condition).unwrap();

            let nodes = query::parser::normalize(py, legacy.as_any()).unwrap();
            assert!(query::evaluator::eval_branch(py, document.as_any(), &nodes).unwrap());

            let canonical = PyDict::new(py);
            let condition = PyDict::new(py);
            condition.set_item("$lt", 20).unwrap();
            canonical.set_item("age", condition).unwrap();

            let nodes = query::parser::normalize(py, canonical.as_any()).unwrap();
            assert!(!query::evaluator::eval_branch(py, document.as_any(), &nodes).unwrap());
        });
    }

    #[test]
    fn operators_follow_python_semantics() {
        Python::initialize();
        Python::attach(|py| {
            let numbers = PyList::new(py, [1i64, 2, 3]).unwrap();
            let required = PyList::new(py, [2i64, 3]).unwrap();
            assert!(
                query::operators::apply(py, "$all", numbers.as_any(), required.as_any()).unwrap()
            );

            let ten = 10i64.into_pyobject(py).unwrap();
            let modulo = PyList::new(py, [3i64, 1]).unwrap();
            assert!(
                query::operators::apply(py, "$mod", ten.as_any(), modulo.as_any()).unwrap()
            );
            assert!(
                query::operators::apply(py, "$regex", "Alice".into_pyobject(py).unwrap().as_any(), "^Ali".into_pyobject(py).unwrap().as_any()).unwrap()
            );
            assert!(!query::operators::apply(
                py,
                "$in",
                "Carol".into_pyobject(py).unwrap().as_any(),
                PyList::new(py, ["Alice", "Bob"]).unwrap().as_any()
            )
            .unwrap());
        });
    }

    #[test]
    fn elem_match_recurses() {
        Python::initialize();
        Python::attach(|py| {
            let first = PyDict::new(py);
            first.set_item("k", "a").unwrap();
            first.set_item("v", 1).unwrap();
            let second = PyDict::new(py);
            second.set_item("k", "a").unwrap();
            second.set_item("v", 9).unwrap();
            let tags = PyList::new(py, [first, second]).unwrap();

            let operand = PyDict::new(py);
            operand.set_item("k", "a").unwrap();
            let threshold = PyDict::new(py);
            threshold.set_item("$gte", 5).unwrap();
            operand.set_item("v", threshold).unwrap();

            assert!(
                query::operators::apply(py, "$elemMatch", tags.as_any(), operand.as_any())
                    .unwrap()
            );
        });
    }
}
