"""
Custom Exception Hierarchy

Provides specific exception types for better error handling and debugging.
"""


class FshareBridgeError(Exception):
    """Base exception for all fshare-bridge errors."""
    
    def __init__(self, message: str, details: dict = None):
        super().__init__(message)
        self.message = message
        self.details = details or {}


# ============================================================================
# Client Errors
# ============================================================================

class ClientError(FshareBridgeError):
    """Base class for client-related errors."""
    pass


class AuthenticationError(ClientError):
    """Raised when authentication fails."""
    pass


class APIError(ClientError):
    """Raised when an API call fails."""
    
    def __init__(self, message: str, status_code: int = None, response: str = None):
        super().__init__(message, {"status_code": status_code, "response": response})
        self.status_code = status_code
        self.response = response


class RateLimitError(APIError):
    """Raised when rate limit is exceeded."""
    pass


class ConnectionError(ClientError):
    """Raised when connection to a service fails."""
    pass


# ============================================================================
# Download Errors
# ============================================================================

class DownloadError(FshareBridgeError):
    """Base class for download-related errors."""
    pass


class DownloadNotFoundError(DownloadError):
    """Raised when a download task is not found."""
    pass


class DownloadAlreadyExistsError(DownloadError):
    """Raised when trying to add a duplicate download."""
    pass


class DownloadFailedError(DownloadError):
    """Raised when a download fails permanently."""
    pass


class InvalidURLError(DownloadError):
    """Raised when a URL is invalid or unsupported."""
    pass


# ============================================================================
# Indexer Errors
# ============================================================================

class IndexerError(FshareBridgeError):
    """Base class for indexer-related errors."""
    pass


class SearchError(IndexerError):
    """Raised when a search operation fails."""
    pass


class ParseError(IndexerError):
    """Raised when parsing search results fails."""
    pass


# ============================================================================
# Configuration Errors
# ============================================================================

class ConfigurationError(FshareBridgeError):
    """Raised when configuration is invalid or missing."""
    pass
