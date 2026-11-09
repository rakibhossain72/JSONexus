"""Validation helpers for JSONexus inputs."""


def validate_collection_name(name: str) -> None:
    """Raise ValueError if name is not a non-empty string."""
    if not isinstance(name, str) or not name:
        raise ValueError("Collection name must be a non-empty string")


def validate_document(doc: Any) -> None:
    """Raise ValueError if doc is not a dict."""
    if not isinstance(doc, dict):
        raise ValueError("Document must be a dict")


def validate_query(query: Any) -> None:
    """Raise ValueError if query is not a dict."""
    if not isinstance(query, dict):
        raise ValueError("Query must be a dict")


def validate_update_fields(fields: Any) -> None:
    """Raise ValueError if fields is not a dict."""
    if not isinstance(fields, dict):
        raise ValueError("Update fields must be a dict")
