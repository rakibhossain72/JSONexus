# Data Modeling in JSONexus

JSONexus allows you to model your data flexibly using JSON documents, providing a schema-less approach to database design.

### 1. Document-based Modeling

JSONexus follows a document-based data model, where each record is represented as a JSON document. This approach allows for flexible schema design and easy scalability.

### 2. Auto-Generated IDs

Every document gets an auto-generated `_id` (uuid4 hex) if one is not provided. The `_id` is always indexed, giving O(1) performance for `get_document`, `updateby_id`, and `deleteby_id`.

```python
result = db.insert('users', {'name': 'Alice', 'age': 30})
# {'status': 'success', 'data': {'_id': '6b72a5d2-6f95-4e49-a152-e24e4415fc0e', 'name': 'Alice', 'age': 30}}
```

### 3. Collections are Top-Level Keys

Collections are stored as top-level keys in the JSON file. Each collection is a list of documents.

```json
{
    "users": [
        {
            "name": "Alice",
            "age": 30,
            "email": "alice@example.com",
            "_id": "6b72a5d2-6f95-4e49-a152-e24e4415fc0e"
        },
        {
            "name": "Bob",
            "age": 25,
            "email": "bob@example.com",
            "_id": "e24e4415fc0e-6f95-4e49-a152-24e4415fc0e"
        },
        {
            "name": "Charlie",
            "age": 35,
            "email": "charlie@example.com",
            "_id": "f85fb55f-10a8-409f-bdd1-ffdc850c67be"
        }
    ]
}
```

### 4. Nested Objects and Arrays

Nested objects and arrays are first-class citizens. There is no limit to nesting depth, and you can query nested fields with dotted paths.

```json
{
    "users": [
        {
            "name": "Alice",
            "address": {"city": "Austin", "state": "TX"},
            "hobbies": ["reading", "hiking"]
        }
    ]
}
```

### 5. Schema-less Flexibility

Documents within the same collection do not need to share the same shape. You can add or remove fields freely without any migration.

### 6. Additional Resources

Refer to the [Operations](./operations.md) guide for CRUD usage, the [Comparison Operators](./comparison-operators.md) for query patterns, and the [Storage Options](./storage-options-in-jsonexus.md) guide for persistence details.