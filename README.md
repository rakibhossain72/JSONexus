# JSONexus

JSONexus is a lightweight and flexible database solution designed for modern applications. It provides a simple and intuitive way to store, query, and manipulate data using JSON documents.

## Key Features

- Document-based storage
- Flexible schema-less design
- Powerful query capabilities
- Easy integration with various platforms

## Installation

You can install JSONexus using pip:

```bash
pip install jsonexus
```

## Usage

```python
from jsonexus import JSONexus

# Create a local database
db = JSONexus('data/db.json')

# Insert a document
db.insert('users', {'name': 'Alice', 'age': 30, 'email': 'alice@example.com'})

# Insert many documents
db.insert_many('users', [
    {'name': 'Bob', 'age': 25},
    {'name': 'Carol', 'age': 35},
])

# Find documents (legacy and canonical operator styles supported)
result = db.find('users', {"age": {'_op': '$gte', '_value': 25}})
result = db.find('users', {"age": {'$gte': 25, '$lte': 40}})

# Update documents by query
db.update('users', {"name": "Alice"}, {'age': 31})

# Update a document by id
doc = db.find_one('users', {"name": "Alice"})
db.updateby_id('users', doc['_id'], {'age': 32})

# Delete documents
db.delete('users', {"name": "Bob"})
db.deleteby_id('users', doc['_id'])

# Count documents
count = db.count('users')

# Exact match (original semantics)
match_result = db.match('users', {"name": "Carol", "age": 35})
```

## Architecture

The engine is written in Rust and bound to Python with PyO3. Documents are kept
as live Python dicts across the FFI boundary, so identity, equality, ordering and
nested JSON values behave exactly as before.

- `src/rust_engine/storage/` — storage backend trait and atomic JSON file backend (temp file + rename).
- `src/rust_engine/query/` — parser, operators, and a single evaluator used by find/update/delete/match. Supports `$eq,$ne,$gt,$gte,$lt,$lte,$in,$nin,$exists,$regex,$size,$all,$mod,$elemMatch`, `$and/$or/$nor`, dotted paths, and legacy `{_op,_value}` style.
- `src/rust_engine/indexing/` — `HashIndex`, `RangeIndex`, and `IndexManager` for lazy per-field auto-indexing.
- `src/rust_engine/collections/` — `Collection` document store (insert, find, update, delete, match, etc.).
- `src/rust_engine/core/` — `JSONexus` coordinator (public API, persistence, flush/close/context manager).
- `jsonexus/` — thin Python re-export shims that keep the existing import paths.

## Performance

- Rust query evaluation, indexing and JSON serialization (no Python round-trip at the FFI boundary).
- Lazy per-field hash and range indexes: O(1) equality lookups and O(log n) range scans (index built on first relevant query).
- Atomic batched writes via temp-file + rename.

Measured on 50,000 documents (median of 5 runs, see `benchmark.py`), compared
with the previous pure-Python engine on CPython 3.14:

| Operation                            | Python  | Rust    | Speedup |
| ------------------------------------ | ------- | ------- | ------- |
| bulk insert_many (50k docs)          | 0.141 s | 0.054 s | 2.60x   |
| equality find (first, builds index)  | 0.036 s | 0.026 s | 1.38x   |
| equality find (second, indexed)      | 0.003 s | 0.003 s | 1.05x   |
| range find ($gte/$lte on age)        | 0.069 s | 0.042 s | 1.66x   |
| update by query (age range)          | 0.024 s | 0.035 s | 0.68x   |
| delete by query (category)           | 0.064 s | 0.066 s | 0.96x   |
| full count (rescan)                  | 0.000 s | 0.000 s | 1.83x   |
| **total of listed operations**       | 0.336 s | 0.226 s | **1.48x** |

## Development

Building the extension requires a Rust toolchain and `maturin`:

```bash
pip install maturin
pip install -e .
pytest tests/
cargo test
cargo clippy --all-targets
python benchmark.py
```

## Documentation

The documentation for JSONexus can be found at https://jsonexus.gitbook.io/jsonexus/.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
