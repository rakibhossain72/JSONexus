# Agent notes

JSONexus is a Rust document-database engine exposed to Python through PyO3.
Python code under `jsonexus/` is a thin re-export layer over
`jsonexus._rust_engine`; engine changes belong in `src/rust_engine/`.

## Build

```bash
pip install maturin
maturin develop            # debug build into the active virtualenv
maturin develop --release  # optimized build (used for benchmarks)
pip install -e .           # regular editable install
cargo build
```

## Test and lint

```bash
python -m pytest tests/ -q
cargo test
cargo clippy --all-targets
python example.py
python benchmark.py
```

`tests/test_jsonexus.py`, `example.py`, and `benchmark.py` are kept unchanged
and must keep passing against the Rust engine.

## Layout

- `src/rust_engine/lib.rs` — PyO3 module entry point.
- `src/rust_engine/core/database.rs` — `JSONexus` coordinator.
- `src/rust_engine/collections/manager.rs` — `Collection`.
- `src/rust_engine/query/` — parser, operators, evaluator.
- `src/rust_engine/indexing/` — hash/range indexes and manager.
- `src/rust_engine/storage/` — backend trait and atomic file storage.
- `jsonexus/` — import-compatible re-export shims.
