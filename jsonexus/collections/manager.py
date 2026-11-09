"""Collection manager.

Implements :class:`Collection`, the core data store for a single JSONexus
collection. It preserves the behavioral semantics of the original
single-file implementation while using the modular query evaluator and
index manager.
"""

from __future__ import annotations

from typing import Any, Iterator

from ..indexing.manager import IndexManager
from ..query.evaluator import EVALUATOR
from ..utils.id_gen import generate_id
from ..utils.validation import (
    validate_document,
    validate_query,
    validate_update_fields,
)


def _ensure_id(doc: dict[str, Any]) -> dict[str, Any]:
    """Ensure ``doc`` has an ``_id``; generate one if missing."""
    if "_id" not in doc:
        doc["_id"] = generate_id()
    return doc


def _doc_equal_ignoring_id(a: dict[str, Any], b: dict[str, Any]) -> bool:
    """Compare two documents ignoring ``_id`` (original semantics)."""
    a_cmp = {k: v for k, v in a.items() if k != "_id"}
    b_cmp = {k: v for k, v in b.items() if k != "_id"}
    return a_cmp == b_cmp


def _indexed_find(
    by_id: dict[str, int],
    documents: list[dict[str, Any]],
    indexes: IndexManager,
    query: dict[str, Any],
) -> list[dict[str, Any]] | None:
    """Attempt an indexed lookup for ``query``.

    Returns the matching documents if an index can serve this query, else
    ``None`` to signal the caller should fall back to a full scan.

    Supported shapes (single-field only):
      * ``{"field": value}``                    -> hash index, O(1)
      * ``{"field": {"$eq": v}}``               -> hash index
      * ``{"field": {"$gt": v}}`` / ``$gte`` / ``$lt`` / ``$lte`` -> range
      * ``{"field": {"_op": "$gt", "_value": v}}`` -> range (legacy)
    """
    if len(query) != 1:
        return None
    field, value = next(iter(query.items()))

    def _resolve(doc_ids: set[str]) -> list[dict[str, Any]]:
        return [documents[by_id[d]] for d in doc_ids if d in by_id]

    # --- equality ---
    if isinstance(value, (str, int, float, bool)) or value is None:
        idx = indexes.get_or_build_hash(field, documents)
        return _resolve(idx.get(value))
    if isinstance(value, dict):
        if "$eq" in value:
            idx = indexes.get_or_build_hash(field, documents)
            return _resolve(idx.get(value["$eq"]))
        if "_op" in value and "_value" in value:
            return _range_or_scan(by_id, documents, indexes, field, value["_op"], value["_value"])
        for op in ("$gt", "$gte", "$lt", "$lte"):
            if op in value:
                return _range_or_scan(by_id, documents, indexes, field, op, value[op])
    return None


def _range_or_scan(
    by_id: dict[str, int],
    documents: list[dict[str, Any]],
    indexes: IndexManager,
    field: str,
    op: str,
    operand: Any,
) -> list[dict[str, Any]] | None:
    """Serve a range query via the range index, or fall back to scan."""
    low: Any | None = None
    high: Any | None = None
    inc_low = True
    inc_high = True
    if op in ("$gt", "$gte"):
        low = operand
        inc_low = op == "$gte"
    elif op in ("$lt", "$lte"):
        high = operand
        inc_high = op == "$lte"
    else:
        return None  # not a range op -> caller scans
    idx = indexes.get_or_build_range(field, documents)
    ids = set(idx.range(low, high, inc_low, inc_high))
    return [documents[by_id[d]] for d in ids if d in by_id]


class Collection:
    """In-memory document collection with query and indexing support."""

    def __init__(self, name: str, documents: list[dict] | None = None) -> None:
        self.name = name
        self._documents: list[dict[str, Any]] = []
        self._by_id: dict[str, int] = {}
        self.indexes = IndexManager()

        for doc in documents or []:
            self._append_document(_ensure_id(dict(doc)))

    # -- internal helpers -------------------------------------------------

    def _append_document(self, doc: dict[str, Any]) -> None:
        idx = len(self._documents)
        self._documents.append(doc)
        self._by_id[str(doc["_id"])] = idx

    def _rebuild_id_index(self) -> None:
        self._by_id = {
            str(doc["_id"]): i
            for i, doc in enumerate(self._documents)
            if "_id" in doc
        }

    def _mutated(self) -> None:
        self.indexes.mark_dirty()

    # -- core API ---------------------------------------------------------

    def insert(self, data: dict[str, Any]) -> dict[str, Any]:
        """Insert a single document."""
        validate_document(data)
        _ensure_id(data)
        self._append_document(data)
        self._mutated()
        return {"status": "success", "data": data}

    def insert_no_duplicate(self, data: dict[str, Any]) -> dict[str, Any]:
        """Insert ``data`` unless an equal document (ignoring ``_id``) exists.

        Preserves original behavior: during comparison, ``_id`` is removed from
        existing items as they are checked.
        """
        validate_document(data)
        for item in self._documents:
            if "_id" in item:
                del item["_id"]
            if item == data:
                return {"status": "duplicate exists", "data": item}

        _ensure_id(data)
        self._append_document(data)
        self._mutated()
        return {"status": "success", "data": data}

    def insert_many(self, data_list: list[dict[str, Any]]) -> dict[str, Any]:
        """Insert multiple documents."""
        if not isinstance(data_list, list):
            raise ValueError("data_list must be a list")
        for item in data_list:
            validate_document(item)
            _ensure_id(item)
            self._append_document(item)
        self._mutated()
        return {"status": "success"}

    def find(self, query: dict[str, Any]) -> list[dict[str, Any]]:
        """Find all documents matching ``query``.

        Uses per-field indexes when available for single-field equality/range
        queries (O(1) / O(log n)); otherwise falls back to a full scan via the
        shared :data:`EVALUATOR`.
        """
        validate_query(query)
        indexed = _indexed_find(self._by_id, self._documents, self.indexes, query)
        if indexed is not None:
            return indexed
        return EVALUATOR.find_all(self._documents, query)

    def find_one(self, query: dict[str, Any]) -> dict[str, Any] | None:
        """Find the first document matching ``query``."""
        validate_query(query)
        results = self.find(query)
        return results[0] if results else None

    def count(self) -> int:
        """Return the number of documents."""
        return len(self._documents)

    def get(self, document_id: Any) -> dict[str, Any] | None:
        """Get a document by ``_id`` in O(1)."""
        idx = self._by_id.get(str(document_id))
        if idx is None:
            return None
        if idx < 0 or idx >= len(self._documents):
            self._rebuild_id_index()
            idx = self._by_id.get(str(document_id))
            if idx is None:
                return None
        doc = self._documents[idx]
        if str(doc.get("_id")) != str(document_id):
            self._rebuild_id_index()
            idx = self._by_id.get(str(document_id))
            return None if idx is None else self._documents[idx]
        return doc

    def update(self, query: dict[str, Any], update_fields: dict[str, Any]) -> dict[str, Any]:
        """Update matching documents with plain dict assignment.

        Preserves original semantics: for simple (non-operator) single-key
        queries, returns after the first matching document update; for
        operator-based queries, updates all matching documents.
        """
        validate_query(query)
        validate_update_fields(update_fields)

        # Detect original "simple query" shape: a single key whose value is a
        # plain str/int (no operator dict). This preserves the early-return
        # behavior of the original implementation.
        simple_single = False
        if len(query) == 1:
            only_value = next(iter(query.values()))
            if isinstance(only_value, (str, int)):
                simple_single = True

        matches = self.find(query)
        for doc in matches:
            for k, v in update_fields.items():
                doc[k] = v
            if simple_single:
                self._mutated()
                return {"status": "success"}

        self._mutated()
        return {"status": "success"}

    def update_by_id(
        self, document_id: Any, update_fields: dict[str, Any]
    ) -> dict[str, Any]:
        """Update a document by ``_id``."""
        validate_update_fields(update_fields)
        doc = self.get(document_id)
        if doc is not None:
            for k, v in update_fields.items():
                doc[k] = v
        self._mutated()
        return {"status": "success"}

    def delete(self, query: dict[str, Any]) -> dict[str, Any]:
        """Delete all documents matching ``query``."""
        validate_query(query)
        matches = self.find(query)
        if matches:
            match_ids = {id(doc) for doc in matches}
            self._documents = [d for d in self._documents if id(d) not in match_ids]
            self._rebuild_id_index()
        self._mutated()
        return {"status": "success"}

    def delete_by_id(self, document_id: Any) -> dict[str, Any]:
        """Delete a document by ``_id``."""
        idx = self._by_id.get(str(document_id))
        if idx is not None and 0 <= idx < len(self._documents):
            doc = self._documents[idx]
            if str(doc.get("_id")) == str(document_id):
                del self._documents[idx]
                self._rebuild_id_index()
        self._mutated()
        return {"status": "success"}

    def match(self, query: dict[str, Any]) -> dict[str, Any]:
        """Exact-match query (original semantics).

        Every key in ``query`` must exist in the document and equal its value.
        Returns ``{'status':'success','result':[...]}`` or
        ``{'status':'failed','result':[]}``.
        """
        validate_query(query)
        matched_items: list[dict[str, Any]] = []
        for item in self._documents:
            match_item = True
            for key, value in query.items():
                if key in item:
                    if item[key] != value:
                        match_item = False
                        break
                else:
                    match_item = False
                    break
            if match_item:
                matched_items.append(item)
        if matched_items:
            return {"status": "success", "result": matched_items}
        return {"status": "failed", "result": []}

    # -- helpers ----------------------------------------------------------

    def to_list(self) -> list[dict[str, Any]]:
        """Return a shallow copy of documents."""
        return list(self._documents)

    def __len__(self) -> int:
        return len(self._documents)

    def __contains__(self, document_id: object) -> bool:
        return str(document_id) in self._by_id

    def __iter__(self) -> Iterator[dict[str, Any]]:
        return iter(self._documents)
