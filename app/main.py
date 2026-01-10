"""
Fshare-Arr Bridge - Main Application
"""

from flask import Flask
import logging
import os
from dotenv import load_dotenv

from .fshare_client import FshareClient
from .pyload_client import PyLoadClient
from .indexer import create_indexer_api
from .sabnzbd import create_sabnzbd_api

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
    
    # Initialize clients
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
    
    # Login to Fshare
    if not fshare_client.login():
        logger.error("Failed to login to Fshare! Check your credentials.")
    
    # Login to pyLoad
    if not pyload_client.login():
        logger.warning("Failed to login to pyLoad! Downloads may not work.")
    
    # Register blueprints
    indexer_bp = create_indexer_api(fshare_client)
    sabnzbd_bp = create_sabnzbd_api(fshare_client, pyload_client)
    
    app.register_blueprint(indexer_bp, url_prefix='/indexer')
    app.register_blueprint(sabnzbd_bp, url_prefix='/sabnzbd')
    
    # Health check endpoint
    @app.route('/health')
    def health():
        return {
            'status': 'healthy',
            'fshare': 'connected' if fshare_client.token else 'disconnected',
            'pyload': 'connected' if pyload_client.logged_in else 'disconnected'
        }
    
    # Root endpoint
    @app.route('/')
    def index():
        return {
            'name': 'Fshare-Arr Bridge',
            'version': '1.0.0',
            'endpoints': {
                'indexer': '/indexer/api',
                'sabnzbd': '/sabnzbd/api',
                'health': '/health'
            }
        }
    
    logger.info("✅ Fshare-Arr Bridge initialized successfully")
    
    return app


if __name__ == '__main__':
    app = create_app()
    
    # Get ports from environment
    indexer_port = int(os.getenv('INDEXER_PORT', 8484))
    
    logger.info(f"Starting Fshare-Arr Bridge on port {indexer_port}")
    logger.info(f"Indexer API: http://0.0.0.0:{indexer_port}/indexer/api")
    logger.info(f"SABnzbd API: http://0.0.0.0:{indexer_port}/sabnzbd/api")
    
    app.run(
        host='0.0.0.0',
        port=indexer_port,
        debug=os.getenv('DEBUG', 'false').lower() == 'true'
    )
