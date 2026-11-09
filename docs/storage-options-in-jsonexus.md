# Storage Options in JSONexus

JSONexus provides a flexible storage design centered on a single JSON file, with a pluggable backend abstraction that allows custom storage implementations.

### 1. File Storage Backend (Default)

By default, JSONexus uses `FileStorageBackend` to persist the database to a single JSON file on the filesystem.

```python
db = JSONexus('data/db.json')
```

### 2. orjson Serialization

All reads and writes use `orjson` for fast, reliable serialization of JSON documents. The on-disk format is plain JSON, so the file can be read by any JSON tooling.

### 3. Atomic Writes

Every write is atomic: JSONexus writes to a temporary file and then atomically replaces the target file via `os.replace`. A crash mid-write never corrupts the database file.

### 4. Auto-Created Parent Directories

Parent directories are created automatically on demand. `data/db.json` works even if the `data/` folder does not exist yet.

### 5. Pluggable StorageBackend ABC

`StorageBackend` is an abstract base class, so you can implement custom backends — for example, an in-memory store, an S3-backed store, or a multi-region sync backend. Implement the load/save/close interface and pass it to `JSONexus`:

```python
from jsonexus.storage import StorageBackend

class MyBackend(StorageBackend):
    ...

db = JSONexus('db', backend=MyBackend())
```

### 6. Lazy In-Memory Model

The loaded dictionary is the source of truth. Collections are wrapped on demand: when you open a file, the raw dict is loaded once; `Collection` objects are created lazily as you access them. Writes are batched and flushed to disk atomically.

### 7. Additional Resources

Refer to the [Operations](./operations.md) guide for CRUD usage, the [Data Modeling](./data-modeling-in-jsonexus.md) guide for schema design, and the [About JSONexus](./about-jsonexus.md) page for architecture details.