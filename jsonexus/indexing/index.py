"""Per-field index structures.

Two index types:

* :class:`HashIndex`  — ``field -> set[document_id]`` for O(1) equality.
* :class:`RangeIndex` — sorted ``(value, doc_id)`` list for O(log n) range.

Indexes are *lazy*: they are built on first access and invalidated on any
mutation to the collection. This keeps insert-heavy workloads fast (no index
maintenance cost) while giving query-heavy workloads O(1)/O(log n) lookups.
"""

from __future__ import annotations

import bisect
from collections import defaultdict
from typing import Any, Iterable

from ..utils.types import Document, DocumentId

# Sentinel for an absent field, distinct from ``None``.
MISSING = object()


class HashIndex:
    """Map field values to the set of document ids that hold them."""

    __slots__ = ("field_path", "_map", "_dirty_docs", "_built")

    def __init__(self, field_path: str) -> None:
        self.field_path = field_path
        self._map: dict[Any, set[DocumentId]] = defaultdict(set)
        self._dirty_docs: set[DocumentId] = set()
        self._built = False
        self._built = False

    # -- mutation ---------------------------------------------------------

    def add(self, doc: Document) -> None:
        value = _path_get(doc, self.field_path)
        if value is not MISSING and _hashable(value):
            self._map[value].add(doc["_id"])
            self._dirty_docs.discard(doc["_id"])

    def remove(self, doc: Document) -> None:
        value = _path_get(doc, self.field_path)
        if value is not MISSING and _hashable(value):
            ids = self._map.get(value)
            if ids is not None:
                ids.discard(doc["_id"])
                if not ids:
                    del self._map[value]
            self._dirty_docs.discard(doc["_id"])

    def update(self, old: Document, new: Document) -> None:
        self.remove(old)
        self.add(new)

    def invalidate(self, doc_id: DocumentId) -> None:
        """Mark a doc as needing re-indexing on next query."""
        self._dirty_docs.add(doc_id)

    def invalidate_all(self) -> None:
        """Drop the index contents; it will be rebuilt on next access."""
        self._map.clear()
        self._dirty_docs.clear()
        self._built = False

    def rebuild(self, documents: Iterable[Document]) -> None:
        self._map.clear()
        self._dirty_docs.clear()
        for doc in documents:
            self.add(doc)
        self._built = True

    # -- query ------------------------------------------------------------

    def get(self, value: Any) -> set[DocumentId]:
        return self._map.get(value, set())

    def is_clean(self, doc_id: DocumentId) -> bool:
        """Whether ``doc_id`` is still correctly indexed."""
        return doc_id not in self._dirty_docs


class RangeIndex:
    """Sorted ``(value, doc_id)`` list for range queries."""

    __slots__ = ("field_path", "_entries", "_built")

    def __init__(self, field_path: str) -> None:
        self.field_path = field_path
        self._entries: list[tuple[Any, DocumentId]] = []
        self._built = False

    def rebuild(self, documents: Iterable[Document]) -> None:
        self._entries = []
        for doc in documents:
            value = _path_get(doc, self.field_path)
            if value is not MISSING and _comparable(value):
                self._entries.append((value, doc["_id"]))
        self._entries.sort(key=lambda item: item[0])
        self._built = True

    def invalidate_all(self) -> None:
        """Drop the index contents; it will be rebuilt on next access."""
        self._entries = []
        self._built = False

    def range(
        self,
        low: Any | None = None,
        high: Any | None = None,
        inclusive_low: bool = True,
        inclusive_high: bool = True,
    ) -> list[DocumentId]:
        entries = self._entries
        if not entries:
            return []
        values = [v for v, _ in entries]

        if low is not None:
            start = bisect.bisect_left(values, low)
            if not inclusive_low:
                start = bisect.bisect_right(values, low)
        else:
            start = 0

        if high is not None:
            end = bisect.bisect_right(values, high)
            if inclusive_high:
                end = bisect.bisect_right(values, high)
            else:
                end = bisect.bisect_left(values, high)
        else:
            end = len(entries)

        if start >= end:
            return []
        return [doc_id for _, doc_id in entries[start:end]]


def _hashable(value: Any) -> bool:
    try:
        hash(value)
        return True
    except TypeError:
        return False


def _comparable(value: Any) -> bool:
    return isinstance(value, (int, float, str)) and not isinstance(value, bool)


def _path_get(document: Document, path: str) -> Any:
    if not path:
        return document
    cur: Any = document
    for part in path.split("."):
        if isinstance(cur, dict) and part in cur:
            cur = cur[part]
        else:
            return MISSING
    return cur