"""
Fshare-Arr-Bridge - Refactored Application

Modern Python package for integrating Fshare.vn with the *arr suite.
"""

__version__ = "2.0.0"
__author__ = "Fshare-Arr-Bridge Contributors"

# Core clients and utilities - always available
from .clients.fshare import FshareClient
from .clients.timfshare import TimFshareClient
from .utils.filename_parser import FilenameParser

# Factory functions - lazy loaded to avoid Flask dependency
def create_indexer_service():
    """Import and create indexer service."""
    from .factory import create_indexer_service as _create
    return _create()

def create_sabnzbd_service():
    """Import and create SABnzbd service (async)."""
    from .factory import create_sabnzbd_service as _create
    return _create()

def create_app():
    """Import and create Flask application."""
    from .app import create_app as _create
    return _create()

def run_app():
    """Import and run Flask application."""
    from .app import run_app as _run
    _run()

__all__ = [
    "FshareClient",
    "TimFshareClient",
    "FilenameParser",
    "create_indexer_service",
    "create_sabnzbd_service",
    "create_app",
    "run_app",
]
