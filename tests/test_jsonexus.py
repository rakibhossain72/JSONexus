"""Test suite for JSONexus (runs with or without pytest)."""

from __future__ import annotations

import os
import sys
import tempfile

# Ensure project root is importable when run as a script.
_ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
if _ROOT not in sys.path:
    sys.path.insert(0, _ROOT)

from jsonexus import JSONexus


# -- helpers ------------------------------------------------------------------

def _db(tmp_path=None):
    """Create a JSONexus instance backed by a temp file."""
    if tmp_path is None:
        fd, path = tempfile.mkstemp(suffix=".json")
        os.close(fd)
        return JSONexus(path), path
    path = os.path.join(str(tmp_path), "test.json")
    return JSONexus(path), path


def _cleanup(path):
    try:
        os.unlink(path)
    except FileNotFoundError:
        pass


# -- insert -------------------------------------------------------------------

def test_insert_returns_success_and_adds_id():
    db, path = _db()
    try:
        res = db.insert("users", {"name": "Alice", "age": 30})
        assert res["status"] == "success"
        assert "_id" in res["data"]
        assert res["data"]["name"] == "Alice"
    finally:
        _cleanup(path)


def test_insert_auto_creates_collection():
    db, path = _db()
    try:
        db.insert("newcol", {"x": 1})
        assert "newcol" in db.get_collections()
    finally:
        _cleanup(path)


def test_insert_many():
    db, path = _db()
    try:
        res = db.insert_many("items", [{"a": 1}, {"a": 2}, {"a": 3}])
        assert res["status"] == "success"
        assert db.count("items") == 3
    finally:
        _cleanup(path)


# -- insert_no_duplicate ------------------------------------------------------

def test_insert_no_duplicate_new():
    db, path = _db()
    try:
        res = db.insert_no_duplicate("users", {"name": "Bob", "age": 25})
        assert res["status"] == "success"
        assert "_id" in res["data"]
    finally:
        _cleanup(path)


def test_insert_no_duplicate_existing():
    db, path = _db()
    try:
        db.insert_no_duplicate("users", {"name": "Bob", "age": 25})
        res = db.insert_no_duplicate("users", {"name": "Bob", "age": 25})
        assert res["status"] == "duplicate exists"
        assert res["data"]["name"] == "Bob"
    finally:
        _cleanup(path)


# -- collections --------------------------------------------------------------

def test_create_collection_success():
    db, path = _db()
    try:
        res = db.create_collection("things")
        assert res == {"status": "success", "name": "things"}
        assert "things" in db.get_collections()
    finally:
        _cleanup(path)


def test_create_collection_already_exists():
    db, path = _db()
    try:
        db.create_collection("things")
        res = db.create_collection("things")
        assert res == {"status": "failed", "message": "Collection already exists"}
    finally:
        _cleanup(path)


def test_get_all():
    db, path = _db()
    try:
        db.insert("a", {"x": 1})
        db.create_collection("b")
        all_data = db.get_all()
        assert "a" in all_data and "b" in all_data
        assert len(all_data["a"]) == 1
    finally:
        _cleanup(path)


def test_get_collection():
    db, path = _db()
    try:
        db.insert("users", {"name": "A"})
        db.insert("users", {"name": "B"})
        docs = db.get_collection("users")
        assert isinstance(docs, list)
        assert len(docs) == 2
    finally:
        _cleanup(path)


def test_get_collections():
    db, path = _db()
    try:
        db.create_collection("c1")
        db.create_collection("c2")
        cols = db.get_collections()
        assert "c1" in cols and "c2" in cols
    finally:
        _cleanup(path)


def test_drop_collection_success():
    db, path = _db()
    try:
        db.create_collection("dropme")
        res = db.drop_collection("dropme")
        assert res == {"status": "success", "name": "dropme"}
        assert "dropme" not in db.get_collections()
    finally:
        _cleanup(path)


def test_drop_collection_not_found():
    db, path = _db()
    try:
        res = db.drop_collection("nope")
        assert res == {"status": "failed", "message": "Collection not found"}
    finally:
        _cleanup(path)


# -- find ---------------------------------------------------------------------

def test_find_scalar_eq():
    db, path = _db()
    try:
        db.insert_many("u", [{"name": "A", "age": 20}, {"name": "B", "age": 30}])
        res = db.find("u", {"name": "A"})
        assert len(res["result"]) == 1
        assert res["result"][0]["age"] == 20
    finally:
        _cleanup(path)


def test_find_legacy_gt():
    db, path = _db()
    try:
        db.insert_many("u", [{"age": 10}, {"age": 20}, {"age": 30}])
        res = db.find("u", {"age": {"_op": "$gt", "_value": 15}})
        ages = sorted(d["age"] for d in res["result"])
        assert ages == [20, 30]
    finally:
        _cleanup(path)


def test_find_canonical_operators():
    db, path = _db()
    try:
        db.insert_many("u", [{"v": 1}, {"v": 5}, {"v": 10}])
        assert len(db.find("u", {"v": {"$gt": 5}})["result"]) == 1
        assert len(db.find("u", {"v": {"$gte": 5}})["result"]) == 2
        assert len(db.find("u", {"v": {"$lt": 5}})["result"]) == 1
        assert len(db.find("u", {"v": {"$lte": 5}})["result"]) == 2
        assert len(db.find("u", {"v": {"$eq": 5}})["result"]) == 1
        assert len(db.find("u", {"v": {"$ne": 5}})["result"]) == 2
    finally:
        _cleanup(path)


def test_find_in_nin():
    db, path = _db()
    try:
        db.insert_many("u", [{"k": "a"}, {"k": "b"}, {"k": "c"}])
        assert len(db.find("u", {"k": {"$in": ["a", "c"]}})["result"]) == 2
        assert len(db.find("u", {"k": {"$nin": ["a", "c"]}})["result"]) == 1
    finally:
        _cleanup(path)


def test_find_regex():
    db, path = _db()
    try:
        db.insert_many("u", [{"name": "Alice"}, {"name": "Bob"}, {"name": "Alicia"}])
        res = db.find("u", {"name": {"$regex": "^Ali"}})
        assert len(res["result"]) == 2
    finally:
        _cleanup(path)


def test_find_exists_true_false():
    db, path = _db()
    try:
        db.insert_many("u", [{"a": 1}, {"b": 2}, {"a": 3, "b": 4}])
        assert len(db.find("u", {"a": {"$exists": True}})["result"]) == 2
        assert len(db.find("u", {"a": {"$exists": False}})["result"]) == 1
    finally:
        _cleanup(path)


def test_find_logical_and():
    db, path = _db()
    try:
        db.insert_many("u", [{"a": 1, "b": 1}, {"a": 1, "b": 2}, {"a": 2, "b": 1}])
        res = db.find("u", {"$and": [{"a": 1}, {"b": 2}]})
        assert len(res["result"]) == 1
    finally:
        _cleanup(path)


def test_find_logical_or():
    db, path = _db()
    try:
        db.insert_many("u", [{"a": 1}, {"a": 2}, {"a": 3}])
        res = db.find("u", {"$or": [{"a": 1}, {"a": 3}]})
        assert len(res["result"]) == 2
    finally:
        _cleanup(path)


def test_find_logical_nor():
    db, path = _db()
    try:
        db.insert_many("u", [{"a": 1}, {"a": 2}, {"a": 3}])
        res = db.find("u", {"$nor": [{"a": 1}, {"a": 3}]})
        assert len(res["result"]) == 1
        assert res["result"][0]["a"] == 2
    finally:
        _cleanup(path)


def test_find_dotted_nested_path():
    db, path = _db()
    try:
        db.insert_many("u", [
            {"user": {"profile": {"city": "NY"}}},
            {"user": {"profile": {"city": "LA"}}},
        ])
        res = db.find("u", {"user.profile.city": "NY"})
        assert len(res["result"]) == 1
    finally:
        _cleanup(path)


def test_find_elemMatch():
    db, path = _db()
    try:
        db.insert_many("u", [
            {"tags": [{"k": "a", "v": 1}, {"k": "b", "v": 2}]},
            {"tags": [{"k": "a", "v": 9}]},
            {"tags": "nope"},
        ])
        res = db.find("u", {"tags": {"$elemMatch": {"k": "a", "v": {"$gte": 5}}}})
        assert len(res["result"]) == 1
        assert res["result"][0]["tags"][0]["v"] == 9
    finally:
        _cleanup(path)


def test_find_size():
    db, path = _db()
    try:
        db.insert_many("u", [{"arr": [1, 2]}, {"arr": [1, 2, 3]}, {"arr": "x"}])
        res = db.find("u", {"arr": {"$size": 2}})
        assert len(res["result"]) == 1
    finally:
        _cleanup(path)


def test_find_all_operator():
    db, path = _db()
    try:
        db.insert_many("u", [{"arr": [1, 2, 3]}, {"arr": [1, 2]}, {"arr": [2, 3, 4]}])
        res = db.find("u", {"arr": {"$all": [2, 3]}})
        assert len(res["result"]) == 2
    finally:
        _cleanup(path)


def test_find_mod():
    db, path = _db()
    try:
        db.insert_many("u", [{"n": 10}, {"n": 11}, {"n": 12}, {"n": 13}])
        res = db.find("u", {"n": {"$mod": [3, 1]}})  # 10%3==1, 13%3==1
        vals = sorted(d["n"] for d in res["result"])
        assert vals == [10, 13]
    finally:
        _cleanup(path)


# -- find_one / count / get ---------------------------------------------------

def test_find_one_returns_dict():
    db, path = _db()
    try:
        db.insert("u", {"name": "Solo"})
        res = db.find_one("u", {"name": "Solo"})
        assert isinstance(res, dict)
        assert res["name"] == "Solo"
    finally:
        _cleanup(path)


def test_find_one_returns_empty_list_when_missing():
    db, path = _db()
    try:
        db.create_collection("u")
        res = db.find_one("u", {"name": "Nobody"})
        assert res == []
    finally:
        _cleanup(path)


def test_count():
    db, path = _db()
    try:
        db.insert_many("u", [{"x": 1}, {"x": 2}, {"x": 3}])
        assert db.count("u") == 3
    finally:
        _cleanup(path)


def test_get_document_found():
    db, path = _db()
    try:
        inserted = db.insert("u", {"name": "X"})["data"]
        doc = db.get_document("u", inserted["_id"])
        assert doc is not None
        assert doc["name"] == "X"
    finally:
        _cleanup(path)


def test_get_document_not_found():
    db, path = _db()
    try:
        db.create_collection("u")
        assert db.get_document("u", "no-such-id") is None
    finally:
        _cleanup(path)


# -- match --------------------------------------------------------------------

def test_match_success_shape():
    db, path = _db()
    try:
        db.insert_many("u", [{"a": 1, "b": 2}, {"a": 1, "b": 3}])
        res = db.match("u", {"a": 1, "b": 2})
        assert res["status"] == "success"
        assert isinstance(res["result"], list)
        assert len(res["result"]) == 1
    finally:
        _cleanup(path)


def test_match_failed_shape():
    db, path = _db()
    try:
        db.insert("u", {"a": 1})
        res = db.match("u", {"a": 999})
        assert res["status"] == "failed"
        assert res["result"] == []
    finally:
        _cleanup(path)


# -- update / delete ----------------------------------------------------------

def test_update_simple_query():
    db, path = _db()
    try:
        db.insert_many("u", [{"name": "A", "age": 1}, {"name": "B", "age": 2}])
        res = db.update("u", {"name": "A"}, {"age": 99})
        assert res["status"] == "success"
        doc = db.find_one("u", {"name": "A"})
        assert doc["age"] == 99
    finally:
        _cleanup(path)


def test_update_operator_query():
    db, path = _db()
    try:
        db.insert_many("u", [{"age": 10}, {"age": 20}, {"age": 30}])
        db.update("u", {"age": {"$gte": 20}}, {"flag": True})
        res = db.find("u", {"flag": True})
        assert len(res["result"]) == 2
    finally:
        _cleanup(path)


def test_update_by_id():
    db, path = _db()
    try:
        inserted = db.insert("u", {"name": "Z", "v": 1})["data"]
        res = db.updateby_id("u", inserted["_id"], {"v": 42})
        assert res["status"] == "success"
        doc = db.get_document("u", inserted["_id"])
        assert doc["v"] == 42
    finally:
        _cleanup(path)


def test_delete_query():
    db, path = _db()
    try:
        db.insert_many("u", [{"t": "keep"}, {"t": "drop"}, {"t": "drop"}])
        res = db.delete("u", {"t": "drop"})
        assert res["status"] == "success"
        assert db.count("u") == 1
        assert db.find_one("u", {"t": "keep"})["t"] == "keep"
    finally:
        _cleanup(path)


def test_delete_by_id():
    db, path = _db()
    try:
        a = db.insert("u", {"name": "a"})["data"]
        b = db.insert("u", {"name": "b"})["data"]
        res = db.deleteby_id("u", a["_id"])
        assert res["status"] == "success"
        assert db.get_document("u", a["_id"]) is None
        assert db.get_document("u", b["_id"]) is not None
    finally:
        _cleanup(path)


# -- persistence --------------------------------------------------------------

def test_persistence_across_instances():
    db1, path = _db()
    try:
        db1.insert("u", {"name": "Persisted", "n": 7})
        db1.insert_many("u", [{"name": "B"}, {"name": "C"}])
        # Open a fresh instance on the same file
        db2 = JSONexus(path)
        assert db2.count("u") == 3
        doc = db2.find_one("u", {"name": "Persisted"})
        assert isinstance(doc, dict) and doc["n"] == 7
    finally:
        _cleanup(path)


# -- validation errors --------------------------------------------------------

def test_validation_bad_collection_name():
    db, path = _db()
    try:
        try:
            db.create_collection("")
            raise AssertionError("expected ValueError")
        except ValueError:
            pass
        try:
            db.create_collection(None)  # type: ignore[arg-type]
            raise AssertionError("expected ValueError")
        except ValueError:
            pass
    finally:
        _cleanup(path)


def test_validation_non_dict_document():
    db, path = _db()
    try:
        try:
            db.insert("u", "not-a-dict")  # type: ignore[arg-type]
            raise AssertionError("expected ValueError")
        except ValueError:
            pass
    finally:
        _cleanup(path)


def test_validation_non_dict_query():
    db, path = _db()
    try:
        db.create_collection("u")
        try:
            db.find("u", "bad")  # type: ignore[arg-type]
            raise AssertionError("expected ValueError")
        except ValueError:
            pass
    finally:
        _cleanup(path)


def test_context_manager_flushes():
    fd, path = tempfile.mkstemp(suffix=".json")
    os.close(fd)
    try:
        with JSONexus(path) as db:
            db.insert("u", {"x": 1})
        db2 = JSONexus(path)
        assert db2.count("u") == 1
    finally:
        _cleanup(path)


if __name__ == "__main__":
    import traceback

    fns = [(n, f) for n, f in sorted(globals().items()) if n.startswith("test_") and callable(f)]
    passed = failed = 0
    for name, fn in fns:
        try:
            fn()
            print(f"PASS {name}")
            passed += 1
        except Exception as e:
            print(f"FAIL {name}: {e}")
            traceback.print_exc()
            failed += 1
    print(f"\n{passed} passed, {failed} failed")
