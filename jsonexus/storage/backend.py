"""Abstract storage backend interface for JSONexus."""

from abc import ABC, abstractmethod


class StorageBackend(ABC):
    """Abstract base class for storage backends."""

    @abstractmethod
    def load(self) -> dict:
        """Load and return the stored data as a dict."""
        raise NotImplementedError

    @abstractmethod
    def save(self, data: dict) -> None:
        """Persist the given data dict to the backend."""
        raise NotImplementedError

    @abstractmethod
    def exists(self) -> bool:
        """Return True if the backend resource exists."""
        raise NotImplementedError

    @abstractmethod
    def delete(self) -> None:
        """Delete the backend resource."""
        raise NotImplementedError
