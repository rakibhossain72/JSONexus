//! Shared type aliases for the Rust engine.

use pyo3::prelude::*;
use pyo3::types::PyDict;

/// A document is kept as the live Python dict object, exactly as the original
/// pure-Python implementation stored it. This preserves object identity,
/// equality, ordering and arbitrary nested JSON values across the FFI boundary.
#[allow(dead_code)]
pub type Document = Py<PyDict>;

/// Document ids are always compared/stringified the way the Python engine did.
pub type DocumentId = String;
