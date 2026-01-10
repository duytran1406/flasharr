"""
Fshare-Arr Bridge - Main Application
"""

from flask import Flask
import logging
import os
from dotenv import load_dotenv

from .fshare_client import FshareClient
from .timfshare_client import TimFshareClient
from .pyload_client import PyLoadClient
from .filename_parser import FilenameNormalizer
from .indexer import create_indexer_api
from .sabnzbd import create_sabnzbd_api
from .web_ui import create_web_ui

# Load environment variables
load_dotenv()

# Configure logging
logging.basicConfig(
    level=logging.DEBUG if os.getenv('DEBUG', 'false').lower() == 'true' else logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)

logger = logging.getLogger(__name__)


def create_app():
    """Create and configure the Flask application"""
    
    app = Flask(__name__)
    
    # Configuration
    app.config['MAX_CONTENT_LENGTH'] = 16 * 1024 * 1024  # 16MB max file size
    app.config['SEND_FILE_MAX_AGE_DEFAULT'] = 0  # Disable caching for development
    
    # Add cache-busting headers
    @app.after_request
    def add_header(response):
        response.headers['Cache-Control'] = 'no-store, no-cache, must-revalidate, post-check=0, pre-check=0, max-age=0'
        response.headers['Pragma'] = 'no-cache'
        response.headers['Expires'] = '-1'
        return response
    
    # Initialize clients
    logger.info("Initializing TimFshare client...")
    timfshare_client = TimFshareClient()
    
    logger.info("Initializing Fshare client...")
    fshare_client = FshareClient(
        email=os.getenv('FSHARE_EMAIL'),
        password=os.getenv('FSHARE_PASSWORD')
    )
    
    logger.info("Initializing pyLoad client...")
    pyload_client = PyLoadClient(
        host=os.getenv('PYLOAD_HOST', 'localhost'),
        port=int(os.getenv('PYLOAD_PORT', 8000)),
        username=os.getenv('PYLOAD_USERNAME', 'admin'),
        password=os.getenv('PYLOAD_PASSWORD')
    )
    
    # Initialize filename normalizer
    logger.info("Initializing filename normalizer...")
    filename_normalizer = FilenameNormalizer()
    
    # Login to Fshare (required for downloading/link generation)
    if not fshare_client.login():
        logger.error("Failed to login to Fshare! Check your credentials.")
    
    # Login to pyLoad
    if not pyload_client.login():
        logger.warning("Failed to login to pyLoad! Downloads may not work.")
    
    # Register blueprints
    # Indexer uses TimFshare for searching
    indexer_bp = create_indexer_api(timfshare_client, filename_normalizer)
    
    # SABnzbd uses FshareClient for link resolution and PyLoad for downloading
    sabnzbd_bp = create_sabnzbd_api(fshare_client, pyload_client, filename_normalizer)
    
    # Web UI uses TimFshare for search and PyLoad for queue
    web_ui_bp = create_web_ui(timfshare_client, pyload_client, filename_normalizer)
    
    app.register_blueprint(indexer_bp, url_prefix='/indexer')
    app.register_blueprint(sabnzbd_bp, url_prefix='/sabnzbd')
    app.register_blueprint(web_ui_bp)  # Web UI at root
    
    # Health check endpoint
    @app.route('/health')
    def health():
        return {
            'status': 'healthy',
            'timfshare': 'connected',
            'fshare': 'connected' if fshare_client.token else 'disconnected',
            'pyload': 'connected' if pyload_client.logged_in else 'disconnected'
        }
    
    logger.info("✅ Fshare-Arr Bridge initialized successfully")
    
    return app


if __name__ == '__main__':
    app = create_app()
    
    # Get port from environment
    port = int(os.getenv('INDEXER_PORT', 8484))
    
    logger.info(f"Starting Fshare-Arr Bridge on port {port}")
    logger.info(f"Web UI: http://0.0.0.0:{port}/")
    logger.info(f"Indexer API: http://0.0.0.0:{port}/indexer/api")
    logger.info(f"SABnzbd API: http://0.0.0.0:{port}/sabnzbd/api")
    
    app.run(
        host='0.0.0.0',
        port=port,
        debug=os.getenv('DEBUG', 'false').lower() == 'true'
    )
