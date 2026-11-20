"""Query evaluator (Rust engine re-export)."""

from jsonexus._rust_engine import QueryEvaluator

# Module-level singleton used by the public API and by ``$elemMatch``.
EVALUATOR = QueryEvaluator()

__all__ = ["EVALUATOR", "QueryEvaluator"]
