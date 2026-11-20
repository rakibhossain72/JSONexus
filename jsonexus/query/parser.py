"""Query parser (Rust engine re-export)."""

from jsonexus._rust_engine import (
    Condition,
    Logical,
    is_leaf,
    is_logical,
    normalize,
)

# Sentinel used when a document field is absent (kept for compatibility with
# code that imports it from this module).
MISSING = object()

__all__ = [
    "Condition",
    "Logical",
    "MISSING",
    "is_leaf",
    "is_logical",
    "normalize",
]
