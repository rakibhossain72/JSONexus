# Operations

### Functions in JSONexus

JSONexus provides various functions to manipulate and query data in the database. All write operations return a status dict, e.g. `{'status': 'success', 'data': {...}}`.

### 1. Insert

Insert a single document into a collection. A `_id` (uuid4 hex) is generated if missing.

```python
result = db.insert('users', {'name': 'Alice', 'age': 30, 'email': 'alice@example.com'})
# {'status': 'success', 'data': {'_id': '...', 'name': 'Alice', 'age': 30, 'email': 'alice@example.com'}}
```

### 2. Insert No Duplicate

Insert a document only if no document with the same `_id` already exists.

```python
result = db.insert_no_duplicate('users', {'_id': 'abc123', 'name': 'Alice'})
```

### 3. Insert Many

Insert multiple documents into a collection in a single batched, atomic write.

```python
result = db.insert_many('users', [
    {'name': 'Alice', 'age': 30, 'email': 'alice@example.com'},
    {'name': 'Bob', 'age': 25, 'email': 'bob@example.com'},
    {'name': 'Charlie', 'age': 35, 'email': 'charlie@example.com'}
])
# {'status': 'success', 'data': [{'_id': '...'}, {'_id': '...'}, {'_id': '...'}]}
```

### 4. Create Collection

Create an empty collection. Returns the empty list if it already exists.

```python
db.create_collection('users')
```

### 5. Get All

Return all documents in a collection.

```python
docs = db.get_all('users')
```

### 6. Get Collection

Get all documents in a collection (alias of `get_all`).

```python
collection = db.get_collection('users')
```

### 7. Get Collections

Return the list of all collection names in the database.

```python
collections = db.get_collections()
```

### 8. Drop Collection

Drop a collection from the database.

```python
db.drop_collection('users')
```

### 9. Find

Find documents that match a specified query. Returns a list of matching documents.

Both the canonical operator style and the legacy `{_op, _value}` style are supported:

```python
# Canonical style
result = db.find('users', {"age": {"$gte": 25, "$lte": 40}})

# Legacy style (still supported)
result = db.find('users', {"age": {'_op': '$gte', '_value': 25}})
```

### 10. Find One

Find a single document that matches the specified query. Returns the document or `None`.

```python
doc = db.find_one('users', {"name": "Alice"})
```

### 11. Count

Count the number of documents in a collection.

```python
count = db.count('users')
```

### 12. Get Document

Get a single document from a collection by its `_id` (O(1) lookup).

```python
document = db.get_document('users', '6b72a5d2-6f95-4e49-a152-e24e4415fc0e')
```

### 13. Match

Exact-match validation. Returns `True` if a document matching all fields exists, `False` otherwise.

```python
match_result = db.match('users', {"name": "Carol", "age": 35})
```

### 14. Update

Update documents that match a specified query. Applies the update to every matching document.

```python
db.update('users', {"name": "Alice"}, {'age': 31})
```

### 15. Update by ID

Update a single document by its `_id`.

```python
doc = db.find_one('users', {"name": "Alice"})
db.updateby_id('users', doc['_id'], {'age': 32})
```

### 16. Delete

Delete documents that match a specified query.

```python
db.delete('users', {"name": "Bob"})
```

### 17. Delete by ID

Delete a single document by its `_id`.

```python
db.deleteby_id('users', doc['_id'])
```

### Additional Resources

Refer to the [Comparison Operators](./comparison-operators.md) for the full operator set, and the [Data Modeling](./data-modeling-in-jsonexus.md) guide for schema design.