"""
Input Validation Module

Provides validation and sanitization functions for user inputs.
"""

import os
import re
import socket
import ipaddress
import logging
from typing import Optional
from urllib.parse import urlparse
from pathlib import Path

logger = logging.getLogger(__name__)


def sanitize_filename(filename: str, max_length: int = 255) -> str:
    """
    Sanitize a filename to prevent path traversal and other attacks.
    
    Security measures:
    - Removes path separators (/, \\)
    - Removes null bytes
    - Removes leading dots (hidden files)
    - Removes control characters
    - Limits length
    - Validates not empty
    
    Args:
        filename: User-provided filename
        max_length: Maximum allowed filename length
        
    Returns:
        Sanitized filename safe for filesystem operations
        
    Raises:
        ValueError: If filename is invalid or empty after sanitization
        
    Examples:
        >>> sanitize_filename("../../etc/passwd")
        "etcpasswd"
        >>> sanitize_filename("file\x00.txt")
        "file.txt"
        >>> sanitize_filename(".hidden")
        "hidden"
    """
    if not filename or not isinstance(filename, str):
        raise ValueError("Filename must be a non-empty string")
    
    # Remove null bytes (can bypass extension checks)
    filename = filename.replace('\x00', '')
    
    # Remove all control characters (0x00-0x1F, 0x7F)
    filename = ''.join(char for char in filename if ord(char) >= 32 and ord(char) != 127)
    
    # Get basename to remove any path components
    filename = os.path.basename(filename)
    
    # Remove leading dots (prevents hidden files and relative paths like ..)
    filename = filename.lstrip('.')
    
    # Remove or replace dangerous characters
    # Keep: alphanumeric, spaces, dots, hyphens, underscores, parentheses, brackets
    filename = re.sub(r'[^\w\s.\-_()\[\]]', '', filename)
    
    # Collapse multiple spaces/dots
    filename = re.sub(r'\.{2,}', '.', filename)  # .. -> .
    filename = re.sub(r'\s{2,}', ' ', filename)  # Multiple spaces -> single
    
    # Trim whitespace
    filename = filename.strip()
    
    # Limit length
    if len(filename) > max_length:
        # Preserve extension if possible
        name, ext = os.path.splitext(filename)
        if ext:
            max_name_length = max_length - len(ext)
            filename = name[:max_name_length] + ext
        else:
            filename = filename[:max_length]
    
    # Final validation
    if not filename:
        raise ValueError("Filename is empty after sanitization")
    
    # Prevent reserved names on Windows
    reserved_names = {
        'CON', 'PRN', 'AUX', 'NUL',
        'COM1', 'COM2', 'COM3', 'COM4', 'COM5', 'COM6', 'COM7', 'COM8', 'COM9',
        'LPT1', 'LPT2', 'LPT3', 'LPT4', 'LPT5', 'LPT6', 'LPT7', 'LPT8', 'LPT9'
    }
    name_without_ext = os.path.splitext(filename)[0].upper()
    if name_without_ext in reserved_names:
        filename = f"file_{filename}"
    
    return filename


def validate_url(url: str, allow_private: bool = False) -> bool:
    """
    Validate URL to prevent SSRF attacks.
    
    Security checks:
    - Only allows http/https schemes
    - Blocks localhost and private IP ranges
    - Validates hostname resolution
    - Prevents DNS rebinding
    
    Args:
        url: URL to validate
        allow_private: If True, allows private IPs (for testing)
        
    Returns:
        True if URL is safe
        
    Raises:
        ValueError: If URL is invalid or dangerous
        
    Examples:
        >>> validate_url("https://example.com/file.zip")
        True
        >>> validate_url("file:///etc/passwd")
        ValueError: Invalid URL scheme: file
        >>> validate_url("http://localhost/admin")
        ValueError: Access to localhost forbidden
    """
    if not url or not isinstance(url, str):
        raise ValueError("URL must be a non-empty string")
    
    # Parse URL
    try:
        parsed = urlparse(url)
    except Exception as e:
        raise ValueError(f"Invalid URL format: {e}")
    
    # Check scheme
    ALLOWED_SCHEMES = ['http', 'https']
    if parsed.scheme not in ALLOWED_SCHEMES:
        raise ValueError(f"Invalid URL scheme: {parsed.scheme}. Only {ALLOWED_SCHEMES} allowed")
    
    # Get hostname
    hostname = parsed.hostname
    if not hostname:
        raise ValueError("URL must contain a hostname")
    
    # Block localhost variations
    BLOCKED_HOSTS = [
        'localhost',
        '127.0.0.1',
        '0.0.0.0',
        '::1',
        '0:0:0:0:0:0:0:1',
    ]
    
    if hostname.lower() in BLOCKED_HOSTS:
        raise ValueError("Access to localhost forbidden")
    
    # Resolve hostname to IP
    try:
        ip_str = socket.gethostbyname(hostname)
        ip_obj = ipaddress.ip_address(ip_str)
    except socket.gaierror:
        raise ValueError(f"Cannot resolve hostname: {hostname}")
    except ValueError as e:
        raise ValueError(f"Invalid IP address: {e}")
    
    # Check if IP is private/reserved (unless explicitly allowed)
    if not allow_private:
        # Check for private networks
        PRIVATE_NETWORKS = [
            ipaddress.ip_network('10.0.0.0/8'),      # Private
            ipaddress.ip_network('172.16.0.0/12'),   # Private
            ipaddress.ip_network('192.168.0.0/16'),  # Private
            ipaddress.ip_network('127.0.0.0/8'),     # Loopback
            ipaddress.ip_network('169.254.0.0/16'),  # Link-local (AWS metadata)
            ipaddress.ip_network('224.0.0.0/4'),     # Multicast
            ipaddress.ip_network('240.0.0.0/4'),     # Reserved
        ]
        
        for network in PRIVATE_NETWORKS:
            if ip_obj in network:
                raise ValueError(f"Access to private network forbidden: {ip_str} in {network}")
    
    # Additional check: prevent DNS rebinding by re-resolving
    # Note: Some domains use round-robin DNS with multiple IPs, so we check if both IPs are public
    try:
        ip_str_2 = socket.gethostbyname(hostname)
        if ip_str != ip_str_2:
            # Check if second IP is also safe
            ip_obj_2 = ipaddress.ip_address(ip_str_2)
            for network in PRIVATE_NETWORKS:
                if ip_obj_2 in network:
                    logger.warning(f"DNS rebinding detected: {hostname} resolved to {ip_str} then {ip_str_2} (private)")
                    raise ValueError("DNS rebinding to private network detected")
            # Both IPs are public, likely round-robin DNS - allow it
            logger.debug(f"Round-robin DNS detected: {hostname} -> {ip_str}, {ip_str_2}")
    except socket.gaierror:
        pass  # Transient DNS error, allow first resolution

    
    return True


def validate_password(password: str) -> bool:
    """
    Validate password complexity.
    
    Requirements:
    - Minimum 12 characters
    - At least one uppercase letter
    - At least one lowercase letter
    - At least one digit
    - At least one special character
    
    Args:
        password: Password to validate
        
    Returns:
        True if password meets requirements
        
    Raises:
        ValueError: If password doesn't meet requirements
    """
    if not password or not isinstance(password, str):
        raise ValueError("Password must be a non-empty string")
    
    if len(password) < 12:
        raise ValueError("Password must be at least 12 characters long")
    
    if not re.search(r'[A-Z]', password):
        raise ValueError("Password must contain at least one uppercase letter")
    
    if not re.search(r'[a-z]', password):
        raise ValueError("Password must contain at least one lowercase letter")
    
    if not re.search(r'[0-9]', password):
        raise ValueError("Password must contain at least one digit")
    
    if not re.search(r'[!@#$%^&*(),.?":{}|<>]', password):
        raise ValueError("Password must contain at least one special character (!@#$%^&*(),.?\":{}|<>)")
    
    return True


def validate_api_key(api_key: str) -> bool:
    """
    Validate API key format.
    
    Args:
        api_key: API key to validate
        
    Returns:
        True if valid format
        
    Raises:
        ValueError: If API key is invalid
    """
    if not api_key or not isinstance(api_key, str):
        raise ValueError("API key must be a non-empty string")
    
    # API keys should be hex strings of reasonable length
    if not re.match(r'^[a-fA-F0-9]{32,128}$', api_key):
        raise ValueError("API key must be a hex string of 32-128 characters")
    
    return True
