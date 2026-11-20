"""ID generation utilities (Rust engine re-export)."""

from jsonexus._rust_engine import generate_id, short_id

__all__ = ["generate_id", "short_id"]
