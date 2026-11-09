"""Index manager for a single collection.

Owns the per-field indexes and decides, per query, whether an index can
short-circuit the scan. Strategy:

* Equality on a single field -> use :class:`HashIndex` if present.
* Range on a single numeric/str field -> use :class:`RangeIndex` if present.
* Anything else (multi-field, ``$or``, ``$regex``) -> fall back to full scan.

Indexes are created lazily on first query and rebuilt automatically after any
mutation. Each index tracks its own staleness via ``_built``; a mutation
invalidates *all* indexes, and each is rebuilt on its next access.
"""

from __future__ import annotations

from typing import Any, Iterable

from ..utils.types import Document, DocumentId
from .index import HashIndex, RangeIndex


class IndexManager:
    """Per-collection index registry."""

    def __init__(self) -> None:
        self._hash: dict[str, HashIndex] = {}
        self._range: dict[str, RangeIndex] = {}

    # -- lifecycle --------------------------------------------------------

    def mark_dirty(self) -> None:
        """Invalidate every index (call after any mutation)."""
        for idx in self._hash.values():
            idx.invalidate_all()
        for idx in self._range.values():
            idx.invalidate_all()

    def reset(self) -> None:
        self._hash.clear()
        self._range.clear()

    def rebuild_all(self, documents: Iterable[Document]) -> None:
        """Rebuild every index from scratch (used after bulk loads)."""
        docs = list(documents)
        for idx in self._hash.values():
            idx.rebuild(docs)
        for idx in self._range.values():
            idx.rebuild(docs)

    # -- index creation ---------------------------------------------------

    def ensure_hash(self, field_path: str) -> HashIndex:
        idx = self._hash.get(field_path)
        if idx is None:
            idx = HashIndex(field_path)
            self._hash[field_path] = idx
        return idx

    def ensure_range(self, field_path: str) -> RangeIndex:
        idx = self._range.get(field_path)
        if idx is None:
            idx = RangeIndex(field_path)
            self._range[field_path] = idx
        return idx

    def get_or_build_hash(self, field_path: str, documents) -> HashIndex:
        """Return the hash index for ``field_path``, building it if stale."""
        idx = self._hash.get(field_path)
        if idx is None:
            idx = HashIndex(field_path)
            self._hash[field_path] = idx
        if not idx._built:
            idx.rebuild(documents)
        return idx

    def get_or_build_range(self, field_path: str, documents) -> RangeIndex:
        idx = self._range.get(field_path)
        if idx is None:
            idx = RangeIndex(field_path)
            self._range[field_path] = idx
        if not idx._built:
            idx.rebuild(documents)
        return idx

    # -- query helpers ----------------------------------------------------

    def try_equality(self, field_path: str, value: Any) -> set[DocumentId] | None:
        """Return indexed doc ids for an equality lookup, or ``None`` if no index."""
        idx = self._hash.get(field_path)
        if idx is None or not idx._built:
            return None
        return idx.get(value)

    def try_range(
        self,
        field_path: str,
        low: Any | None,
        high: Any | None,
        inclusive_low: bool,
        inclusive_high: bool,
    ) -> list[DocumentId] | None:
        idx = self._range.get(field_path)
        if idx is None or not idx._built:
            return None
        return idx.range(low, high, inclusive_low, inclusive_high)