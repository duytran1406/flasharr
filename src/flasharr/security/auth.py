"""
Authentication Module

Provides authentication middleware and API key management.
"""

import secrets
import logging
from functools import wraps
from typing import Optional, Callable
from flask import request, jsonify, current_app

logger = logging.getLogger(__name__)


def generate_api_key(length: int = 32) -> str:
    """
    Generate a cryptographically secure API key.
    
    Args:
        length: Length of the key in bytes (default 32 = 64 hex chars)
        
    Returns:
        Hex-encoded API key
    """
    return secrets.token_hex(length)


def require_auth(f: Callable) -> Callable:
    """
    Decorator to require API key authentication.
    
    Checks for API key in:
    1. X-API-Key header
    2. Authorization: Bearer <key> header
    3. api_key query parameter (less secure, for compatibility)
    
    Usage:
        @api_bp.route("/protected")
        @require_auth
        def protected_endpoint():
            return jsonify({"data": "secret"})
    """
    @wraps(f)
    def decorated_function(*args, **kwargs):
        # Get configured API key
        configured_key = current_app.config.get('API_KEY')
        
        # If no API key is configured, allow access (backward compatibility)
        # In production, this should be required
        if not configured_key:
            logger.warning("API_KEY not configured - authentication disabled!")
            return f(*args, **kwargs)
        
        # Check X-API-Key header
        api_key = request.headers.get('X-API-Key')
        
        # Check Authorization header
        if not api_key:
            auth_header = request.headers.get('Authorization', '')
            if auth_header.startswith('Bearer '):
                api_key = auth_header[7:]  # Remove 'Bearer ' prefix
        
        # Check query parameter (least secure, but sometimes needed)
        if not api_key:
            api_key = request.args.get('api_key')
        
        # Validate API key using constant-time comparison
        if not api_key or not secrets.compare_digest(api_key, configured_key):
            logger.warning(f"Unauthorized API access attempt from {request.remote_addr}")
            return jsonify({
                "status": "error",
                "message": "Unauthorized - Invalid or missing API key",
                "code": "UNAUTHORIZED"
            }), 401
        
        # Authentication successful
        return f(*args, **kwargs)
    
    return decorated_function


def optional_auth(f: Callable) -> Callable:
    """
    Decorator for endpoints that work with or without authentication.
    Sets request.authenticated = True if valid API key provided.
    
    Usage:
        @api_bp.route("/public")
        @optional_auth
        def public_endpoint():
            if hasattr(request, 'authenticated') and request.authenticated:
                # Provide enhanced response
                pass
    """
    @wraps(f)
    def decorated_function(*args, **kwargs):
        configured_key = current_app.config.get('API_KEY')
        
        if configured_key:
            api_key = request.headers.get('X-API-Key')
            
            if not api_key:
                auth_header = request.headers.get('Authorization', '')
                if auth_header.startswith('Bearer '):
                    api_key = auth_header[7:]
            
            if api_key and secrets.compare_digest(api_key, configured_key):
                request.authenticated = True
            else:
                request.authenticated = False
        else:
            request.authenticated = False
        
        return f(*args, **kwargs)
    
    return decorated_function
