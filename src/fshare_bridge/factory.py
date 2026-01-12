"""
Fshare-Arr-Bridge Factory

Factory functions for creating properly configured service instances.
"""

import asyncio
import logging
from typing import Optional

from .clients.fshare import FshareClient
from .clients.timfshare import TimFshareClient
from .downloader.engine import DownloadEngine
from .downloader.builtin_client import BuiltinDownloadClient
from .services.indexer import IndexerService
from .services.sabnzbd import SABnzbdEmulator
from .utils.filename_parser import FilenameParser
from .core.config import get_config

logger = logging.getLogger(__name__)


def create_indexer_service() -> IndexerService:
    """
    Create a configured indexer service.
    
    Returns:
        IndexerService instance
    """
    search_client = TimFshareClient()
    parser = FilenameParser()
    
    return IndexerService(
        search_client=search_client,
        parser=parser,
    )


async def create_sabnzbd_service() -> SABnzbdEmulator:
    """
    Create a configured SABnzbd emulator service with built-in download engine.
    
    Returns:
        SABnzbdEmulator instance
    """
    config = get_config()
    
    # Create Fshare client
    fshare_client = FshareClient.from_config(config.fshare)
    await asyncio.to_thread(fshare_client.login)
    
    # Create download engine
    engine = DownloadEngine(max_concurrent=3)
    await engine.start()
    
    # Create built-in download client adapter
    download_client = BuiltinDownloadClient(
        fshare_client=fshare_client,
        engine=engine,
        download_dir=config.download.download_dir,
    )
    
    # Create filename parser
    parser = FilenameParser()
    
    # Create SABnzbd emulator
    emulator = SABnzbdEmulator(
        fshare_client=fshare_client,
        download_client=download_client,
        parser=parser,
    )
    
    logger.info("✅ SABnzbd emulator created with built-in download engine")
    return emulator


def create_all_services():
    """
    Create all services for the application.
    
    Returns:
        Dictionary with all service instances
    """
    # Create indexer synchronously
    indexer = create_indexer_service()
    
    # SABnzbd emulator needs async initialization
    # This should be called from an async context
    logger.info("Services created - SABnzbd requires async initialization")
    
    return {
        "indexer": indexer,
        "create_sabnzbd": create_sabnzbd_service,  # Function to call in async context
    }
