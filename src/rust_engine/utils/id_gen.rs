//! Identifier generation, mirroring `uuid.uuid4()` output.

use pyo3::prelude::*;
use uuid::Uuid;

/// Full-length UUID v4 string, identical in shape to `str(uuid.uuid4())`.
pub fn generate_id() -> String {
    Uuid::new_v4().to_string()
}

/// Compact 8-character identifier, identical to `uuid.uuid4().hex[:8]`.
pub fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

#[pyfunction(name = "generate_id")]
pub fn generate_id_py() -> String {
    generate_id()
}

#[pyfunction(name = "short_id")]
pub fn short_id_py() -> String {
    short_id()
}
