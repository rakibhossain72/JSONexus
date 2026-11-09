"""Query parser.

Normalizes the various supported query shapes into a uniform list of
``Condition`` objects consumed by :mod:`jsonexus.query.evaluator`.

Supported shapes
---------------
* ``{"field": value}``                        -> ``$eq`` against ``value``
* ``{"field": {"_op": "$gt", "_value": 5}}``  -> legacy JSONexus style
* ``{"field": {"$gt": 5}}``                   -> canonical MongoDB style
* ``{"$and": [q, q]}`` / ``"$or"`` / ``"$nor"`` -> logical combinators
* ``"a.b"`` dotted keys                       -> nested field access
"""

from __future__ import annotations

from dataclasses import dataclass, field
from typing import Any

from ..utils.types import Query

# Sentinel used when a document field is absent. Distinct from ``None`` so
# that ``{"field": None}`` queries are distinguishable from missing keys.
MISSING = object()


@dataclass(frozen=True, slots=True)
class Condition:
    """A single leaf predicate: ``(field_path, operator, operand)``."""

    field_path: str
    operator: str
    operand: Any


@dataclass(frozen=True, slots=True)
class Logical:
    """A logical combinator over a list of normalized conditions."""

    op: str  # "$and" | "$or" | "$nor"
    conditions: list  # list[Condition | Logical]


def _split_field_path(path: str) -> list[str]:
    return path.split(".") if path else []


def _normalize_leaf(field: str, value: Any) -> Condition:
    """Normalize a single ``{field: value}`` entry into a leaf ``Condition``."""
    if isinstance(value, dict):
        # Legacy style: {"_op": "$gt", "_value": 5}
        if "_op" in value and "_value" in value:
            return Condition(field, value["_op"], value["_value"])
        # Canonical style: {"$gt": 5, "$lt": 10} -> take first operator.
        # Multi-operator on one field is rare; we pick the first known op.
        for op, operand in value.items():
            if op in _KNOWN_OPERATORS:
                return Condition(field, op, operand)
        raise ValueError(f"Unknown operator shape for field {field!r}: {value!r}")
    # Scalar -> equality.
    return Condition(field, "$eq", value)


def normalize(query: Query) -> list:
    """Parse a raw query dict into a flat list of ``Condition``/``Logical``.

    Logical combinators (``$and``/``$or``/``$nor``) are returned as nested
    ``Logical`` nodes so the evaluator can short-circuit.
    """
    if not isinstance(query, dict):
        raise ValueError("Query must be a dict")

    result: list = []
    for key, value in query.items():
        if key in ("$and", "$or", "$nor"):
            if not isinstance(value, list):
                raise ValueError(f"{key} expects a list operand")
            result.append(Logical(key, [normalize(q) for q in value]))
        else:
            result.append(_normalize_leaf(key, value))
    return result


# Operators that may appear as keys in the canonical ``{op: operand}`` form.
_KNOWN_OPERATORS = frozenset(
    {
        "$eq", "$ne", "$gt", "$gte", "$lt", "$lte",
        "$in", "$nin", "$exists", "$regex", "$size",
        "$all", "$mod", "$elemMatch",
    }
)


def is_logical(node: Any) -> bool:
    return isinstance(node, Logical)


def is_leaf(node: Any) -> bool:
    return isinstance(node, Condition)