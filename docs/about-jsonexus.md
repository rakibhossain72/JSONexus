# About JSONexus

JSONexus is a lightweight and flexible database solution designed for modern applications. It provides a simple and intuitive way to store, query, and manipulate data using JSON documents.

### Key Features

* Document-based storage
* Flexible schema-less design
* Powerful query capabilities
* Easy integration with various platforms
* Atomic writes and auto-created parent directories
* Lazy per-field auto-indexing for fast queries

### Modular Architecture (v1.1.0)

The package was refactored from a single 323-line file into a modular package. Each subpackage has a single, focused responsibility:

* `jsonexus/utils/` — shared types, ID generation, and input validation.
* `jsonexus/storage/` — `StorageBackend` ABC and `FileStorageBackend` with atomic writes (temp file + `os.replace`) using `orjson`.
* `jsonexus/query/` — query parser, operator implementations, and a single shared `EVALUATOR` used by all query paths. Supports `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, `$lte`, `$in`, `$nin`, `$exists`, `$regex`, `$size`, `$all`, `$mod`, `$elemMatch`, `$and`/`$or`/`$nor`, dotted paths, and the legacy `{_op, _value}` style.
* `jsonexus/indexing/` — `HashIndex`, `RangeIndex`, and `IndexManager` for lazy per-field auto-indexing.
* `jsonexus/collections/` — `Collection` document store (insert, find, update, delete, match, etc.).
* `jsonexus/core/` — `JSONexus` database coordinator (public API, persistence, flush/close, context manager).

### Performance Design

* **orjson** serialization for fast reads and writes.
* **Lazy per-field hash and range indexes**: O(1) equality lookups and O(log n) range scans. Indexes are built on the first relevant query, so insert-heavy workloads stay fast.
* **Single shared query evaluator** — deduplicated from the original four copies — used by find, update, delete, and match paths.
* **Atomic batched writes** via temp-file + `os.replace`.

Measured on 50,000 documents (see `benchmark.py`):

| Operation                                     | Seconds |
| --------------------------------------------- | ------- |
| bulk insert_many (50k docs)                   | 0.13    |
| equality find (first, builds index)           | 0.035   |
| equality find (second, indexed)               | 0.002   |
| range find ($gte/$lte on age)                 | 0.066   |
| update by query (age range)                   | 0.024   |
| delete by query (category)                    | 0.065   |
| full count (rescan)                           | 0.00003 |

### Backward Compatibility

The public API is fully backward compatible. Existing code continues to work, and both the canonical operator style (`{"age": {"$gte": 25}}`) and the legacy style (`{"age": {"_op": "$gte", "_value": 25}}`) are supported.

### Testing and Benchmarks

The test suite covers 43 cases across the modular components. `benchmark.py` reproduces the performance measurements above.

### GitHub Repository

JSONexus is an open-source project hosted on GitHub. You can find the source code, contribute to the project, and report issues on the GitHub repository:

[JSONexus GitHub Repository](https://github.com/rakibmia7254/jsonexus)

### Additional Resources

Refer to the [documentation](./) for detailed usage instructions, API reference, and examples.