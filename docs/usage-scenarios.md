# Usage Scenarios

JSONexus is a versatile database solution suitable for various usage scenarios across different domains.

### 1. Web Applications

Use JSONexus as the backend database for web applications. Store user data, product information, session details, and more.

### 2. Mobile Apps

Integrate JSONexus into mobile apps to store user profiles, settings, local cache data, and other application data.

### 3. IoT Devices

Deploy JSONexus on IoT devices to manage sensor data, device configurations, telemetry data, and remote monitoring.

### 4. Prototyping and Testing

Rapidly prototype and test applications using JSONexus. Its schema-less nature allows for quick iterations and experimentation.

### 5. Education and Learning

Teach database concepts, data modeling, and query languages using JSONexus in educational settings. Students can experiment with real-world data scenarios.

### 6. High-Volume Inserts

Bulk-load data efficiently with `insert_many`, which performs a single atomic batched write. Combined with lazy per-field indexing, indexes are only built when a query actually needs them, keeping insert throughput high.

```python
docs = [{'name': f'user_{i}', 'age': i % 50} for i in range(10000)]
db.insert_many('users', docs)
```

### 7. Query-Heavy Workloads

For read-heavy workloads, JSONexus auto-indexes fields on the first relevant query. On a 50,000-document dataset, an equality `find` drops from ~35 ms (first pass, builds index) to ~2 ms (indexed) — roughly a **17x** speedup. Range scans use a B-tree-style range index for O(log n) performance.

```python
db.find('users', {"age": {"$gte": 25, "$lte": 40}})
```

### 8. Nested / Relational-ish Data

Store nested objects and arrays as first-class JSON values. Query nested fields with dotted paths, and apply sub-queries to array elements with `$elemMatch`.

```python
db.insert('users', {
    'name': 'Alice',
    'address': {'city': 'Austin', 'state': 'TX'},
    'hobbies': ['reading', 'hiking', 'coding']
})

# Dotted path query
db.find('users', {"address.city": "Austin"})

# Array element match
db.find('users', {"hobbies": {"$elemMatch": {"$regex": "^h"}}})
```

### 9. Exact-Match Validation

Use `match` to verify that a document with exactly the given fields exists. Returns `True` or `False`.

```python
db.match('users', {"name": "Carol", "age": 35})
```

### 10. Persistence Across Processes

The database lives in a single JSON file. Reopen the same file from another process or script and your data is right there.

```python
# Process 1
db = JSONexus('data/db.json')
db.insert('users', {'name': 'Alice'})

# Process 2 (later)
db = JSONexus('data/db.json')
db.find('users', {"name": "Alice"})  # finds Alice
```

### 11. Context-Manager Usage

Use the context manager to guarantee the database is flushed and closed safely, even if an exception occurs:

```python
with JSONexus('data/db.json') as db:
    db.insert('users', {'name': 'Alice', 'age': 30})
    db.insert('users', {'name': 'Bob', 'age': 25})
# Data is persisted on exit
```

### Additional Resources

Refer to the [documentation](./) for detailed usage instructions, API reference, and examples.