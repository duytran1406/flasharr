"""
Flask Application Factory

Creates and configures the Flask application.
"""

import logging
import time
import os
import secrets
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
    # Support both old structure (app/) and new structure (src/flasharr/web/)
    base_dir = Path("/app")
    
    # Check for legacy structure in standard Docker path
    if (base_dir / "app" / "templates").exists():
        template_folder = str(base_dir / "app" / "templates")
        static_folder = str(base_dir / "app" / "static")
    # Check for root templates (if running from /app and templates are at root)
    elif (base_dir / "templates").exists():
        template_folder = str(base_dir / "templates")
        static_folder = str(base_dir / "static")
    # Development/Package fallback
    else:
        pkg_base = Path(__file__).parent
        # If running from src/flasharr/web, templates are up one level in src/flasharr/templates
        if (pkg_base.parent / "templates").exists():
            template_folder = str(pkg_base.parent / "templates")
            static_folder = str(pkg_base.parent / "static")
        else:
            # Fallback to local (if web/templates existed)
            template_folder = str(pkg_base / "templates")
            static_folder = str(pkg_base / "static")
    
    app = Flask(
        __name__,
        template_folder=template_folder,
        static_folder=static_folder,
    )
    
    # Load configuration
    config = get_config()
    app.config["DEBUG"] = config.server.debug
    
    # Secure secret key from environment or generate one
    secret_key = os.getenv("FLASK_SECRET_KEY")
    if not secret_key:
        logger.warning("FLASK_SECRET_KEY not set, generating random key (sessions will not persist across restarts)")
        secret_key = secrets.token_hex(32)
    app.config["SECRET_KEY"] = secret_key
    
    # API key for authentication (optional, for backward compatibility)
    app.config["API_KEY"] = config.server.api_key
    
    # Apply overrides
    if config_override:
        app.config.update(config_override)
    
    # Register blueprints
    from .api import api_bp
    from .routes import main_bp
    from .indexer_routes import indexer_bp
    from .sabnzbd_routes import sabnzbd_bp
    from .settings_api import settings_bp
    from .integration_routes import integration_bp
    from .tmdb_routes import tmdb_bp
    from .discovery_routes import discovery_bp
    
    # Initialize Core Services (Critical for Gunicorn/Prod)
    try:
        from ..core.account_manager import AccountManager
        from ..factory import create_indexer_service
        
        # Attach services to app instance so they are accessible via current_app
        app.account_manager = AccountManager()
        app.indexer = create_indexer_service() 
        # Note: sabnzbd service is usually async/background, might need special handling if used here
        # For now, verify-account only needs account_manager
        
        logger.info(f"Core services initialized: AccountManager, Indexer")
    except Exception as e:
        logger.error(f"Failed to initialize core services: {e}")
    
    # Register blueprints
    
    # Inject version into all templates
    @app.context_processor
    def inject_version():
        version = "1.0.0-alpha"
        try:
            with open("/app/VERSION", "r") as f:
                version = f.read().strip()
        except:
            pass
        return dict(version=version, now=int(time.time()))

    # Register Blueprints (actual registration)
    app.register_blueprint(main_bp)
    app.register_blueprint(api_bp, url_prefix="/api")
    app.register_blueprint(settings_bp, url_prefix="/api/settings")
    app.register_blueprint(indexer_bp, url_prefix="/indexer")
    app.register_blueprint(sabnzbd_bp, url_prefix="/sabnzbd")
    app.register_blueprint(integration_bp, url_prefix="/api")
    app.register_blueprint(tmdb_bp, url_prefix="/api/tmdb")
    app.register_blueprint(discovery_bp, url_prefix="/api/discovery")
    
    # Setup logging
    if not app.debug:
        logging.basicConfig(
            level=logging.INFO,
            format="%(asctime)s - %(name)s - %(levelname)s - %(message)s",
        )
    
    logger.info(f"Flask app created: templates={template_folder}")
    
    return app
