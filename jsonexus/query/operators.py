"""Operator registry (Rust engine re-export)."""

from jsonexus._rust_engine import (
    op_all,
    op_elem_match,
    op_eq,
    op_exists,
    op_gt,
    op_gte,
    op_in,
    op_lt,
    op_lte,
    op_mod,
    op_ne,
    op_nin,
    op_regex,
    op_size,
)

OPERATORS = {
    "$eq": op_eq,
    "$ne": op_ne,
    "$gt": op_gt,
    "$gte": op_gte,
    "$lt": op_lt,
    "$lte": op_lte,
    "$in": op_in,
    "$nin": op_nin,
    "$exists": op_exists,
    "$regex": op_regex,
    "$size": op_size,
    "$all": op_all,
    "$mod": op_mod,
    "$elemMatch": op_elem_match,
}

# Aliases for the legacy ``_op`` / ``_value`` query style.
LEGACY_ALIASES = {
    "$eq": "$eq",
    "$ne": "$ne",
    "$gt": "$gt",
    "$gte": "$gte",
    "$lt": "$lt",
    "$lte": "$lte",
}


def get_operator(name):
    """Return the operator callable for ``name`` or ``None`` if unknown."""
    return OPERATORS.get(name)


__all__ = ["LEGACY_ALIASES", "OPERATORS", "get_operator"]
