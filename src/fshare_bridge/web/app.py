"""
Flask Application Factory

Creates and configures the Flask application.
"""

import logging
from pathlib import Path
from flask import Flask

from ..core.config import get_config

logger = logging.getLogger(__name__)


def create_app(config_override: dict = None) -> Flask:
    """
    Create and configure the Flask application.
    
    Args:
        config_override: Optional configuration overrides
        
    Returns:
        Configured Flask application instance
    """
    # Determine template and static paths
    # Support both old structure (app/) and new structure (src/fshare_bridge/web/)
    base_path = Path(__file__).parent
    legacy_path = Path("/app")
    
    if (legacy_path / "templates").exists():
        template_folder = str(legacy_path / "templates")
        static_folder = str(legacy_path / "static")
    else:
        template_folder = str(base_path / "templates")
        static_folder = str(base_path / "static")
    
    app = Flask(
        __name__,
        template_folder=template_folder,
        static_folder=static_folder,
    )
    
    # Load configuration
    config = get_config()
    app.config["DEBUG"] = config.server.debug
    app.config["SECRET_KEY"] = "fshare-bridge-secret-key"
    
    # Apply overrides
    if config_override:
        app.config.update(config_override)
    
    # Register blueprints
    from .api import api_bp
    from .routes import main_bp
    from .indexer_routes import indexer_bp
    from .sabnzbd_routes import sabnzbd_bp
    
    app.register_blueprint(main_bp)
    app.register_blueprint(api_bp, url_prefix="/api")
    app.register_blueprint(indexer_bp, url_prefix="/indexer")
    app.register_blueprint(sabnzbd_bp, url_prefix="/sabnzbd")
    
    # Setup logging
    if not app.debug:
        logging.basicConfig(
            level=logging.INFO,
            format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        )
    
    logger.info(f"Flask app created: templates={template_folder}")
    
    return app
