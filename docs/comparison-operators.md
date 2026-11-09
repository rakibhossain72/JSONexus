# Comparison Operators

JSONexus supports a rich set of query operators. Queries are JSON objects where each key is a field name and the value is either a scalar (equality) or an operator object.

## Operator Reference

### 1. Equality

Match values that are equal to a specified value.

```python
# Scalar equality
db.find('users', {"name": "Alice"})

# Explicit $eq
db.find('users', {"age": {"$eq": 30}})

# Not equal
db.find('users', {"age": {"$ne": 25}})
```

### 2. Range

Match values within a numeric range.

```python
# Greater than
db.find('users', {"age": {"$gt": 25}})

# Greater than or equal
db.find('users', {"age": {"$gte": 25}})

# Less than
db.find('users', {"age": {"$lt": 40}})

# Less than or equal
db.find('users', {"age": {"$lte": 40}})

# Range combination
db.find('users', {"age": {"$gte": 25, "$lte": 40}})
```

### 3. Set

Match values that are in / not in a set.

```python
# In a set
db.find('users', {"age": {"$in": [25, 30, 35]}})

# Not in a set
db.find('users', {"age": {"$nin": [25, 30]}})
```

### 4. Logical

Combine sub-queries with logical operators. Each takes a list of sub-queries.

```python
# And
db.find('users', {"$and": [{"age": {"$gte": 25}}, {"age": {"$lte": 40}}]})

# Or
db.find('users', {"$or": [{"name": "Alice"}, {"name": "Bob"}]})

# Nor
db.find('users', {"$nor": [{"age": {"$lt": 25}}, {"age": {"$gt": 40}}]})
```

### 5. Field Tests

Test for the presence or absence of a field.

```python
# Field exists
db.find('users', {"email": {"$exists": True}})

# Field does not exist
db.find('users', {"email": {"$exists": False}})
```

### 6. Pattern

Match values against a regular expression.

```python
db.find('users', {"email": {"$regex": ".*@example\\.com"}})
```

### 7. Array

Query array fields.

```python
# Array size
db.find('posts', {"tags": {"$size": 3}})

# All elements present
db.find('posts', {"tags": {"$all": ["python", "json"]}})

# Element match (sub-query applied to each element)
db.find('posts', {"comments": {"$elemMatch": {"author": "Alice"}}})
```

### 8. Math

Match values where `value % divisor == remainder`.

```python
db.find('numbers', {"value": {"$mod": [3, 0]}})
```

### 9. Dotted Path

Query nested fields using dot notation.

```python
db.find('users', {"address.city": "Austin"})
```

## Legacy Operator Style

The legacy `{_op, _value}` style is still supported for `$eq`, `$ne`, `$gt`, `$gte`, `$lt`, and `$lte`:

```python
# Legacy
db.find('users', {"age": {'_op': '$gte', '_value': 25}})

# Equivalent canonical
db.find('users', {"age": {"$gte": 25}})
```

## Comparison with SQL Queries

Here's how you would achieve the same result in SQL:

```sql

[SQL] SELECT * FROM users WHERE age > 25;
[JSONexus] db.find('users', {"age": {"$gt": 25}})

[SQL] SELECT * FROM users WHERE age >= 25;
[JSONexus] db.find('users', {"age": {"$gte": 25}})

[SQL] SELECT * FROM users WHERE age < 25;
[JSONexus] db.find('users', {"age": {"$lt": 25}})

[SQL] SELECT * FROM users WHERE age <= 25;
[JSONexus] db.find('users', {"age": {"$lte": 25}})

[SQL] SELECT * FROM users WHERE age != 25;
[JSONexus] db.find('users', {"age": {"$ne": 25}})

[SQL] SELECT * FROM users WHERE age = 25;
[JSONexus] db.find('users', {"age": {"$eq": 25}})

```

Both JSONexus queries and SQL queries provide ways to filter data based on certain conditions, but JSONexus queries use a JSON-like syntax, while SQL queries use a more structured SQL syntax.