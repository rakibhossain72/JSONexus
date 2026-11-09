"""Query evaluator.

Evaluates normalized conditions (from :mod:`jsonexus.query.parser`) against
documents. This is the single hot path shared by ``find``/``find_one``/
``update``/``delete``/``match`` — the duplicated logic in the original
``__init__.py`` is gone.

Optimizations
-------------
* Dotted field-path access via a cached ``_path_get`` helper.
* Short-circuiting on the first failing condition for ``$and``-style queries.
* ``$or`` short-circuits on the first matching branch.
* ``$elemMatch`` reuses this evaluator through a lazy singleton hook.
"""

from __future__ import annotations

from typing import Any, Iterable

from ..utils.types import Document
from .operators import get_operator
from .parser import MISSING, Condition, Logical, is_leaf, normalize


class QueryEvaluator:
    """Stateless evaluator. Reused across all query operations."""

    __slots__ = ()

    # -- public API --------------------------------------------------------

    def matches(self, document: Document, query: Any) -> bool:
        """Return ``True`` if ``document`` satisfies ``query``."""
        conditions = normalize(query) if isinstance(query, dict) else query
        return self._eval_branch(document, conditions)

    def find_all(
        self,
        documents: Iterable[Document],
        query: Any,
    ) -> list[Document]:
        """Filter ``documents`` to those matching ``query``."""
        conditions = normalize(query) if isinstance(query, dict) else query
        out: list[Document] = []
        for doc in documents:
            if self._eval_branch(doc, conditions):
                out.append(doc)
        return out

    def find_one(
        self,
        documents: Iterable[Document],
        query: Any,
    ) -> Document | None:
        """Return the first matching document, or ``None``."""
        conditions = normalize(query) if isinstance(query, dict) else query
        for doc in documents:
            if self._eval_branch(doc, conditions):
                return doc
        return None

    # -- internals --------------------------------------------------------

    def _eval_branch(self, document: Document, conditions: list) -> bool:
        for node in conditions:
            if is_leaf(node):
                if not self._eval_leaf(document, node):
                    return False
            else:  # Logical
                if not self._eval_logical(document, node):
                    return False
        return True

    def _eval_logical(self, document: Document, node: Logical) -> bool:
        op = node.op
        if op == "$and":
            for branch in node.conditions:
                if not self._eval_branch(document, branch):
                    return False
            return True
        if op == "$or":
            for branch in node.conditions:
                if self._eval_branch(document, branch):
                    return True
            return False
        if op == "$nor":
            for branch in node.conditions:
                if self._eval_branch(document, branch):
                    return False
            return True
        return False

    def _eval_leaf(self, document: Document, leaf: Condition) -> bool:
        op = leaf.operator
        operand = leaf.operand

        # Special operators that don't read a document field.
        if op == "$exists":
            value = _path_get(document, leaf.field_path)
            return (value is not MISSING) == bool(operand)

        value = _path_get(document, leaf.field_path)
        if value is MISSING:
            # Missing fields never match comparison operators except $exists
            # (handled above) and $ne (which matches anything that isn't equal).
            return op == "$ne"

        fn = get_operator(op)
        if fn is None:
            raise ValueError(f"Unknown operator: {op}")
        return fn(value, operand)


# ---------------------------------------------------------------------------
# Field-path access
# ---------------------------------------------------------------------------

def _path_get(document: Document, path: str) -> Any:
    """Get a (possibly dotted) field from a document.

    Returns the ``MISSING`` sentinel when any segment is absent.
    """
    if not path:
        return document
    cur: Any = document
    for part in path.split("."):
        if isinstance(cur, dict) and part in cur:
            cur = cur[part]
        else:
            return MISSING
    return cur


# Module-level singleton used by the public API and by ``$elemMatch``.
EVALUATOR = QueryEvaluator()

# Wire the operator singleton so ``$elemMatch`` can recurse.
from . import operators as _operators  # noqa: E402

_operators._EVALUATOR_INSTANCE.instance = EVALUATOR  # type: ignore[attr-defined]