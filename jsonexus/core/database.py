"""JSONexus coordinator.

Ties the storage backend and :class:`~jsonexus.collections.manager.Collection`
layer together into the public :class:`JSONexus` API.

The loaded database dict (``self._raw``) is the source of truth. Collections
are wrapped lazily and cached in ``self._collections``; a wrapped Collection
mutates the same list object stored in ``self._raw[name]``, so persistence
simply saves ``self._raw``.
"""

from __future__ import annotations

from typing import Any

from ..collections.manager import Collection
from ..indexing.manager import IndexManager
from ..storage.backend import StorageBackend
from ..storage.file_backend import FileStorageBackend
from ..utils.validation import validate_collection_name


class JSONexus:
    """Top-level coordinator for a JSONexus database.

    Parameters
    ----------
    db_name:
        Path to the database file.
    storage_backend:
        Optional storage backend. Defaults to
        :class:`~jsonexus.storage.file_backend.FileStorageBackend`.
    """

    def __init__(
        self, db_name: str, storage_backend: StorageBackend | None = None
    ) -> None:
        self.db_name = db_name
        self._backend = storage_backend or FileStorageBackend(db_name)
        self._raw: dict[str, Any] = self._backend.load()
        self._collections: dict[str, Collection] = {}

    # -- internal -----------------------------------------------------------

    def _get_collection(self, collection_name: str) -> Collection:
        validate_collection_name(collection_name)
        if collection_name not in self._collections:
            raw_list = self._raw[collection_name]
            collection = Collection.__new__(Collection)
            collection.name = collection_name
            collection._documents = raw_list
            collection._by_id = {
                str(doc["_id"]): i
                for i, doc in enumerate(raw_list)
                if "_id" in doc
            }
            collection.indexes = IndexManager()
            self._collections[collection_name] = collection
        return self._collections[collection_name]

    def _ensure_collection(self, collection_name: str) -> Collection:
        if collection_name not in self._raw:
            self._raw[collection_name] = []
        return self._get_collection(collection_name)

    def _persist(self) -> None:
        self._backend.save(self._raw)

    # -- public API ---------------------------------------------------------

    def insert(self, collection_name: str, data: dict[str, Any]) -> dict[str, Any]:
        collection = self._ensure_collection(collection_name)
        result = collection.insert(data)
        self._persist()
        return result

    def insert_no_duplicate(
        self, collection_name: str, data: dict[str, Any]
    ) -> dict[str, Any]:
        collection = self._ensure_collection(collection_name)
        result = collection.insert_no_duplicate(data)
        self._persist()
        return result

    def insert_many(
        self, collection_name: str, data: list[dict[str, Any]]
    ) -> dict[str, Any]:
        collection = self._ensure_collection(collection_name)
        result = collection.insert_many(data)
        self._persist()
        return result

    def create_collection(self, collection_name: str) -> dict[str, Any]:
        validate_collection_name(collection_name)
        if collection_name in self._raw:
            return {"status": "failed", "message": "Collection already exists"}
        self._raw[collection_name] = []
        self._persist()
        return {"status": "success", "name": collection_name}

    def get_all(self) -> dict[str, Any]:
        return self._raw

    def get_collection(self, collection_name: str) -> list[dict[str, Any]]:
        return self._get_collection(collection_name).to_list()

    def get_collections(self) -> list[str]:
        return list(self._raw.keys())

    def drop_collection(self, collection_name: str) -> dict[str, Any]:
        validate_collection_name(collection_name)
        if collection_name not in self._raw:
            return {"status": "failed", "message": "Collection not found"}
        del self._raw[collection_name]
        self._collections.pop(collection_name, None)
        self._persist()
        return {"status": "success", "name": collection_name}

    def find(self, collection_name: str, query: dict[str, Any]) -> dict[str, Any]:
        return {"result": self._get_collection(collection_name).find(query)}

    def find_one(
        self, collection_name: str, query: dict[str, Any]
    ) -> dict[str, Any] | list:
        doc = self._get_collection(collection_name).find_one(query)
        return doc if doc is not None else []

    def count(self, collection_name: str) -> int:
        return self._get_collection(collection_name).count()

    def get_document(
        self, collection_name: str, document_id: str
    ) -> dict[str, Any] | None:
        return self._get_collection(collection_name).get(document_id)

    def match(self, collection_name: str, query: dict[str, Any]) -> dict[str, Any]:
        return self._get_collection(collection_name).match(query)

    def update(
        self,
        collection_name: str,
        query: dict[str, Any],
        update_fields: dict[str, Any],
    ) -> dict[str, Any]:
        result = self._get_collection(collection_name).update(query, update_fields)
        self._persist()
        return result

    def updateby_id(
        self,
        collection_name: str,
        document_id: str,
        update_fields: dict[str, Any],
    ) -> dict[str, Any]:
        result = self._get_collection(collection_name).update_by_id(
            document_id, update_fields
        )
        self._persist()
        return result

    def delete(self, collection_name: str, query: dict[str, Any]) -> dict[str, Any]:
        result = self._get_collection(collection_name).delete(query)
        self._persist()
        return result

    def deleteby_id(
        self, collection_name: str, document_id: str
    ) -> dict[str, Any]:
        result = self._get_collection(collection_name).delete_by_id(document_id)
        self._persist()
        return result

    # -- lifecycle ----------------------------------------------------------

    def flush(self) -> None:
        self._persist()

    def close(self) -> None:
        self.flush()

    def __enter__(self) -> JSONexus:
        return self

    def __exit__(self, *args: Any) -> None:
        self.close()
