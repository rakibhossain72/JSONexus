"""Benchmark for JSONexus.

Creates a temp database, inserts 50,000 documents via insert_many, and times
key operations. Cleans up the temp file on exit.

Run: python benchmark.py
"""

from __future__ import annotations

import os
import tempfile
import time

from jsonexus import JSONexus


def _time(label: str, fn):
    start = time.perf_counter()
    result = fn()
    elapsed = time.perf_counter() - start
    return label, elapsed, result


def main() -> None:
    fd, path = tempfile.mkstemp(suffix=".json", prefix="jsonexus_bench_")
    os.close(fd)
    rows = []
    try:
        db = JSONexus(path)

        # Build dataset
        docs = [
            {"name": f"user_{i}", "age": i % 100, "category": f"cat_{i % 10}"}
            for i in range(50_000)
        ]

        label, sec, _ = _time(
            "bulk insert_many (50k docs)",
            lambda: db.insert_many("bench", docs),
        )
        rows.append((label, sec))

        # Equality find: first call builds auto-index, second is indexed
        label, sec, res = _time(
            "equality find (first, builds index)",
            lambda: db.find("bench", {"category": "cat_3"}),
        )
        rows.append((label, sec))
        count_first = len(res["result"])

        label, sec, res = _time(
            "equality find (second, indexed)",
            lambda: db.find("bench", {"category": "cat_3"}),
        )
        rows.append((label, sec))
        assert len(res["result"]) == count_first

        # Range find
        label, sec, res = _time(
            "range find ($gte/$lte on age)",
            lambda: db.find("bench", {"age": {"$gte": 25, "$lte": 35}}),
        )
        rows.append((label, sec))
        range_count = len(res["result"])

        # Update by query
        label, sec, _ = _time(
            "update by query (age range)",
            lambda: db.update("bench", {"age": {"$gte": 90}}, {"senior": True}),
        )
        rows.append((label, sec))

        # Delete by query
        label, sec, _ = _time(
            "delete by query (category)",
            lambda: db.delete("bench", {"category": "cat_9"}),
        )
        rows.append((label, sec))

        # Full rescan count
        label, sec, count = _time(
            "full count (rescan)",
            lambda: db.count("bench"),
        )
        rows.append((label, sec))

        # Print results
        print("\nJSONexus benchmark (50,000 documents)")
        print("=" * 56)
        print(f"{'Operation':<40} {'Seconds':>12}")
        print("-" * 56)
        for op, secs in rows:
            print(f"{op:<40} {secs:>12.6f}")
        print("-" * 56)
        print(f"{'Documents remaining':<40} {count:>12}")
        print(f"{'Range match count (25-35)':<40} {range_count:>12}")
        print(f"{'Equality match count (cat_3)':<40} {count_first:>12}")

    finally:
        try:
            os.unlink(path)
        except FileNotFoundError:
            pass


if __name__ == "__main__":
    main()
