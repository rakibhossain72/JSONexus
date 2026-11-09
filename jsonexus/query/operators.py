"""Operator registry for query predicates.

Each operator is a callable ``(field_value, operand) -> bool``. The registry
maps the canonical MongoDB-style operator names (``$eq``, ``$gt`` ...) to
their implementations. Operators are intentionally tiny and branch-free where
possible so the hot path through :mod:`jsonexus.query.evaluator` stays fast.
"""

from __future__ import annotations

import re
from typing import Any, Callable

OperatorFn = Callable[[Any, Any], bool]


def _eq(field: Any, operand: Any) -> bool:
    return field == operand


def _ne(field: Any, operand: Any) -> bool:
    return field != operand


def _gt(field: Any, operand: Any) -> bool:
    try:
        return field > operand
    except TypeError:
        return False


def _gte(field: Any, operand: Any) -> bool:
    try:
        return field >= operand
    except TypeError:
        return False


def _lt(field: Any, operand: Any) -> bool:
    try:
        return field < operand
    except TypeError:
        return False


def _lte(field: Any, operand: Any) -> bool:
    try:
        return field <= operand
    except TypeError:
        return False


def _in(field: Any, operand: Any) -> bool:
    try:
        return field in operand
    except TypeError:
        return False


def _nin(field: Any, operand: Any) -> bool:
    try:
        return field not in operand
    except TypeError:
        return False


def _exists(field: Any, operand: Any) -> bool:
    # ``field`` is a sentinel when the key is absent; see evaluator.
    from .parser import MISSING

    present = field is not MISSING
    return present == bool(operand)


def _regex(field: Any, operand: Any) -> bool:
    if not isinstance(field, str) or not isinstance(operand, str):
        return False
    return _compile(operand).search(field) is not None


def _size(field: Any, operand: Any) -> bool:
    try:
        return len(field) == operand
    except TypeError:
        return False


def _all(field: Any, operand: Any) -> bool:
    try:
        return all(item in field for item in operand)
    except TypeError:
        return False


def _mod(field: Any, operand: Any) -> bool:
    try:
        divisor, remainder = operand
        return field % divisor == remainder
    except (TypeError, ZeroDivisionError, ValueError):
        return False


def _elem_match(field: Any, operand: Any) -> bool:
    if not isinstance(field, list):
        return False
    if not isinstance(operand, dict):
        return False
    # Defer to the evaluator to avoid a circular import at module load.
    evaluator = _EVALUATOR_INSTANCE.instance
    if evaluator is None:
        raise RuntimeError("$elemMatch evaluator not initialized")
    return any(evaluator.matches(item, operand) for item in field if isinstance(item, dict))


class _EvaluatorShim:
    """Lazy singleton holder so ``$elemMatch`` can reuse the evaluator."""

    instance: "QueryEvaluator | None" = None  # type: ignore[name-defined]


_EVALUATOR_INSTANCE = _EvaluatorShim()


def _compile(pattern: str) -> "re.Pattern[str]":
    cache = _regex.cache  # type: ignore[attr-defined]
    compiled = cache.get(pattern)
    if compiled is None:
        compiled = re.compile(pattern)
        cache[pattern] = compiled
    return compiled


_regex.cache = {}  # type: ignore[attr-defined]


OPERATORS: dict[str, OperatorFn] = {
    "$eq": _eq,
    "$ne": _ne,
    "$gt": _gt,
    "$gte": _gte,
    "$lt": _lt,
    "$lte": _lte,
    "$in": _in,
    "$nin": _nin,
    "$exists": _exists,
    "$regex": _regex,
    "$size": _size,
    "$all": _all,
    "$mod": _mod,
    "$elemMatch": _elem_match,
}

# Aliases for the legacy ``_op`` / ``_value`` query style used by earlier
# JSONexus releases. These map directly onto the canonical operators.
LEGACY_ALIASES: dict[str, str] = {
    "$eq": "$eq",
    "$ne": "$ne",
    "$gt": "$gt",
    "$gte": "$gte",
    "$lt": "$lt",
    "$lte": "$lte",
}


def get_operator(name: str) -> OperatorFn | None:
    """Return the operator callable for ``name`` or ``None`` if unknown."""
    return OPERATORS.get(name)