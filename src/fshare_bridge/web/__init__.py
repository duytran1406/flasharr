# Web Module
"""Flask web application: routes, API, and templates."""

from .app import create_app
from .api import api_bp
from .routes import main_bp

__all__ = ["create_app", "api_bp", "main_bp"]
