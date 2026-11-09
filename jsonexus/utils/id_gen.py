"""ID generation utilities for JSONexus."""

import uuid


def generate_id() -> str:
    """Generate a full-length unique identifier as a hex string."""
    return str(uuid.uuid4())


def short_id() -> str:
    """Generate a compact 8-character unique identifier."""
    return uuid.uuid4().hex[:8]
