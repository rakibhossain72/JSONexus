"""Type aliases used across the JSONexus package."""

from typing import Any

Document = dict[str, Any]
DocumentId = str
CollectionName = str
FieldName = str
Query = dict[str, Any]
Update = dict[str, Any]
OperatorResult = bool
