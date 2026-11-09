"""Collection layer for JSONexus.

Provides the :class:`Collection` class, a lightweight in-memory document
store for a single named collection with query, mutation, and indexing
support.
"""

from .manager import Collection

__all__ = ["Collection"]
