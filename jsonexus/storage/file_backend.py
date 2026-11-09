"""File-based storage backend implementation for JSONexus."""

import os
import tempfile

import orjson

from jsonexus.storage.backend import StorageBackend


class FileStorageBackend(StorageBackend):
    """Storage backend that reads/writes a JSON file on disk."""

    def __init__(self, path: str) -> None:
        self.path = path

    def load(self) -> dict:
        """Load the JSON file. Returns {} if missing or empty."""
        if not self.exists():
            return {}
        with open(self.path, "rb") as f:
            raw = f.read()
        if not raw:
            return {}
        return orjson.loads(raw)

    def save(self, data: dict) -> None:
        """Atomically write data to the JSON file.

        The parent directory is created if it does not exist, so a path like
        ``data/db.json`` works even when ``data/`` has not been created yet.
        """
        payload = orjson.dumps(data, option=orjson.OPT_INDENT_2)
        dir_name = os.path.dirname(self.path) or "."
        os.makedirs(dir_name, exist_ok=True)
        fd, tmp_path = tempfile.mkstemp(dir=dir_name, suffix=".tmp")
        try:
            with os.fdopen(fd, "wb") as f:
                f.write(payload)
            os.replace(tmp_path, self.path)
        except Exception:
            if os.path.exists(tmp_path):
                os.unlink(tmp_path)
            raise

    def exists(self) -> bool:
        """Return True if the file exists."""
        return os.path.exists(self.path)

    def delete(self) -> None:
        """Delete the file if it exists."""
        if self.exists():
            os.unlink(self.path)
