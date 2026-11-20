import json
from pathlib import Path

from jsonexus import JSONexus


DB_PATH = Path("data/db.json")


def section(title):
    print()
    print("=" * 72)
    print(f"  {title}")
    print("=" * 72)


def show(label, value):
    print(f"\n{label}:")
    print(json.dumps(value, indent=2, default=str))


# Ensure a clean, reproducible run
if DB_PATH.exists():
    DB_PATH.unlink()
if DB_PATH.parent.exists():
    # keep directory removal localized and safe for this demo
    try:
        if not any(DB_PATH.parent.iterdir()):
            DB_PATH.parent.rmdir()
    except Exception:
        pass

section("1. Creating a database")
db = JSONexus(str(DB_PATH))
print(f"Database path: {DB_PATH}")
print(f"File exists after init: {DB_PATH.exists()}")

section("2. Create collection")
result = db.create_collection("users")
show("create_collection('users')", result)
dup = db.create_collection("users")
show("create_collection('users') duplicate", dup)

section("3. Insert (single document)")
alice = db.insert(
    "users", {"name": "Alice", "age": 30, "email": "alice@example.com"}
)
show("insert('users', Alice)", alice)
alice_id = alice["data"]["_id"]

section("4. Insert many")
many = db.insert_many(
    "users",
    [
        {"name": "Bob", "age": 25, "email": "bob@example.com"},
        {"name": "Carol", "age": 35, "email": "carol@example.com"},
        {"name": "Dave", "age": 40, "email": "dave@example.com"},
        {"name": "Eve", "age": 22, "email": "eve@example.com"},
    ],
)
show("insert_many('users', [Bob, Carol, Dave, Eve])", many)
show("count('users') after insert_many", db.count("users"))

section("5. get_collections / get_collection / get_all")
show("get_collections()", db.get_collections())
show("get_collection('users') [first 2 shown]", db.get_collection("users")[:2])
all_data = db.get_all()
show(
    "get_all() summary",
    {name: {"document_count": len(docs)} for name, docs in all_data.items()},
)

section("6. find — scalar equality")
show("find(users, {'name': 'Alice'})", db.find("users", {"name": "Alice"}))

section("7. find — legacy operator style")
show(
    "find(users, {'age': {'_op': '$gte', '_value': 25}})",
    db.find("users", {"age": {"_op": "$gte", "_value": 25}}),
)

section("8. find — canonical operator style (multi-operator on one field)")
show(
    "find(users, {'age': {'$gte': 25, '$lte': 40}})",
    db.find("users", {"age": {"$gte": 25, "$lte": 40}}),
)

section("9. find — $in, $nin, $regex, $exists, $and, $or, $nor")
show(
    "find(users, {'name': {'$in': ['Alice', 'Bob']}})",
    db.find("users", {"name": {"$in": ["Alice", "Bob"]}}),
)
show(
    "find(users, {'name': {'$nin': ['Alice', 'Bob']}})",
    db.find("users", {"name": {"$nin": ["Alice", "Bob"]}}),
)
show(
    "find(users, {'email': {'$regex': 'example\\\\.com$'}})",
    db.find("users", {"email": {"$regex": r"example\.com$"}}),
)
show(
    "find(users, {'nickname': {'$exists': False}})",
    db.find("users", {"nickname": {"$exists": False}}),
)
show(
    "find(users, {'$and': [{'age': {'$gte': 25}}, {'name': {'$ne': 'Alice'}}]})",
    db.find("users", {"$and": [{"age": {"$gte": 25}}, {"name": {"$ne": "Alice"}}]}),
)
show(
    "find(users, {'$or': [{'name': 'Alice'}, {'name': 'Eve'}]})",
    db.find("users", {"$or": [{"name": "Alice"}, {"name": "Eve"}]}),
)
show(
    "find(users, {'$nor': [{'name': 'Alice'}, {'name': 'Bob'}]})",
    db.find("users", {"$nor": [{"name": "Alice"}, {"name": "Bob"}]}),
)

section("10. find — dotted nested path")
db.insert(
    "users",
    {
        "name": "Frank",
        "age": 29,
        "email": "frank@example.com",
        "address": {"city": "Austin", "zip": "78701"},
    },
)
show(
    "find(users, {'address.city': 'Austin'})",
    db.find("users", {"address.city": "Austin"}),
)

section("11. find_one — found and not found")
show("find_one(users, {'name': 'Carol'})", db.find_one("users", {"name": "Carol"}))
show(
    "find_one(users, {'name': 'Nobody'})",
    db.find_one("users", {"name": "Nobody"}),
)

section("12. count — before and after a delete")
show("count(users) before delete", db.count("users"))
show("delete(users, {'name': 'Eve'})", db.delete("users", {"name": "Eve"}))
show("count(users) after delete", db.count("users"))

section("13. get_document by _id — found and not found")
show("get_document(users, alice_id)", db.get_document("users", alice_id))
show(
    "get_document(users, 'does-not-exist')",
    db.get_document("users", "does-not-exist"),
)

section("14. match — exact-match semantics")
show(
    "match(users, {'name': 'Carol', 'age': 35})",
    db.match("users", {"name": "Carol", "age": 35}),
)
show(
    "match(users, {'name': 'Carol', 'age': 99})",
    db.match("users", {"name": "Carol", "age": 99}),
)

section("15. update — simple query and operator query")
show(
    "update(users, {'name': 'Alice'}, {'age': 31}) [simple query]",
    db.update("users", {"name": "Alice"}, {"age": 31}),
)
show(
    "re-find Alice after simple update",
    db.find("users", {"name": "Alice"}),
)
show(
    "update(users, {'age': {'$gte': 35}}, {'tier': 'senior'}) [operator query]",
    db.update("users", {"age": {"$gte": 35}}, {"tier": "senior"}),
)
show(
    "re-find tier='senior' after operator update",
    db.find("users", {"tier": "senior"}),
)

section("16. updateby_id — update by _id and confirm")
show(
    "updateby_id(users, alice_id, {'age': 32, 'city': 'NYC'})",
    db.updateby_id("users", alice_id, {"age": 32, "city": "NYC"}),
)
show(
    "get_document(users, alice_id) after updateby_id",
    db.get_document("users", alice_id),
)

section("17. delete — by query and deleteby_id")
frank = db.find_one("users", {"name": "Frank"})
frank_id = frank["_id"] if isinstance(frank, dict) else None
show("delete(users, {'name': 'Bob'})", db.delete("users", {"name": "Bob"}))
show(
    "deleteby_id(users, frank_id)",
    db.deleteby_id("users", frank_id) if frank_id else {"status": "skipped"},
)
show("find(users, {'name': 'Frank'}) after deleteby_id", db.find("users", {"name": "Frank"}))

section("18. Persistence — reopen same file in a second instance")
db2 = JSONexus(str(DB_PATH))
show("count(users) via second instance", db2.count("users"))
show("find(users, {'name': 'Alice'}) via second instance", db2.find("users", {"name": "Alice"}))

section("19. Context manager")
try:
    with JSONexus(str(DB_PATH)) as db3:
        ctx_count = db3.count("users")
        show("count(users) inside context manager", ctx_count)
        show("insert inside context manager", db3.insert("users", {"name": "Grace", "age": 27}))
        show("count(users) after insert inside context manager", db3.count("users"))
    print("\nContext manager exited cleanly (flush/close).")
    # Verify persisted change
    db4 = JSONexus(str(DB_PATH))
    show("count(users) after context manager (new instance)", db4.count("users"))
except Exception as e:
    print(f"\nContext manager error: {e}")

section("20. Validation errors")
cases = [
    ("bad collection name (empty string)", lambda: db.create_collection("")),
    ("non-dict document", lambda: db.insert("users", "not-a-dict")),
    ("non-dict query", lambda: db.find("users", "not-a-dict")),
]
for label, fn in cases:
    try:
        fn()
        print(f"\n{label}: NO ERROR (unexpected)")
    except ValueError as e:
        print(f"\n{label}:")
        print(f"  ValueError: {e}")

section("21. Final summary")
print(f"\nCollections: {db4.get_collections()}")
print(f"Total documents left in 'users': {db4.count('users')}")
print("\nExample completed successfully.")
