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

- `jsonexus/utils/` — shared types, ID generation, and input validation.
- `jsonexus/storage/` — storage backend ABC and file backend with atomic writes (temp file + `os.replace`) using `orjson`.
- `jsonexus/query/` — query parser, operator implementations, and a single shared evaluator (`EVALUATOR`). Supports `$eq,$ne,$gt,$gte,$lt,$lte,$in,$nin,$exists,$regex,$size,$all,$mod,$elemMatch`, `$and/$or/$nor`, dotted paths, and legacy `{_op,_value}` style.
- `jsonexus/indexing/` — `HashIndex`, `RangeIndex`, and `IndexManager` for lazy per-field auto-indexing.
- `jsonexus/collections/` — `Collection` document store (insert, find, update, delete, match, etc.).
- `jsonexus/core/` — `JSONexus` database coordinator (public API, persistence, flush/close/context manager).

## Performance

- `orjson` serialization for fast reads/writes.
- Lazy per-field hash and range indexes: O(1) equality lookups and O(log n) range scans (index built on first relevant query). Measured on 50,000 documents, an equality `find` drops from **35 ms (first pass, builds index) to 2 ms (indexed)** — roughly a **17x** speedup.
- Single shared query evaluator (deduplicated from the original four copies) used by find/update/delete/match paths.
- Atomic batched writes via temp-file + `os.replace`.

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

## Development

```bash
pip install -e .
pytest tests/
python benchmark.py
```

## Documentation

The documentation for JSONexus can be found at https://jsonexus.gitbook.io/jsonexus/.

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.
