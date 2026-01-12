"""
Formatting Utilities

Helper functions for formatting sizes, speeds, durations, and Newznab XML.
"""

from typing import Union, List, Tuple, Optional
from xml.etree import ElementTree as ET


def format_size(size_bytes: Union[int, float], precision: int = 2) -> str:
    """
    Format bytes to human-readable size string.
    
    Args:
        size_bytes: Size in bytes
        precision: Number of decimal places
        
    Returns:
        Formatted string like "1.5 GB"
        
    Examples:
        >>> format_size(1024)
        '1.00 KB'
        >>> format_size(1536, precision=1)
        '1.5 KB'
    """
    if size_bytes == 0:
        return "0 B"
    
    units = ["B", "KB", "MB", "GB", "TB", "PB"]
    unit_index = 0
    size = float(size_bytes)
    
    while size >= 1024 and unit_index < len(units) - 1:
        size /= 1024
        unit_index += 1
    
    return f"{size:.{precision}f} {units[unit_index]}"


def format_speed(bytes_per_second: Union[int, float], precision: int = 2) -> str:
    """
    Format bytes/second to human-readable speed string.
    
    Args:
        bytes_per_second: Speed in bytes per second
        precision: Number of decimal places
        
    Returns:
        Formatted string like "1.5 MB/s"
        
    Examples:
        >>> format_speed(1048576)
        '1.00 MB/s'
    """
    if bytes_per_second == 0:
        return "0 B/s"
    
    return format_size(bytes_per_second, precision).replace(" ", " ") + "/s"


def format_duration(seconds: Union[int, float]) -> str:
    """
    Format seconds to human-readable duration string.
    
    Args:
        seconds: Duration in seconds
        
    Returns:
        Formatted string like "1h 23m 45s" or "45s"
        
    Examples:
        >>> format_duration(3661)
        '1h 1m 1s'
        >>> format_duration(45)
        '45s'
    """
    if seconds <= 0:
        return "0s"
    
    seconds = int(seconds)
    
    days = seconds // 86400
    hours = (seconds % 86400) // 3600
    minutes = (seconds % 3600) // 60
    secs = seconds % 60
    
    parts = []
    if days > 0:
        parts.append(f"{days}d")
    if hours > 0:
        parts.append(f"{hours}h")
    if minutes > 0:
        parts.append(f"{minutes}m")
    if secs > 0 or not parts:
        parts.append(f"{secs}s")
    
    return " ".join(parts)


def format_eta(seconds: Union[int, float]) -> str:
    """
    Format ETA seconds to HH:MM:SS or MM:SS format.
    
    Args:
        seconds: ETA in seconds
        
    Returns:
        Formatted string like "01:23:45" or "23:45"
    """
    if seconds <= 0:
        return "--:--"
    
    seconds = int(seconds)
    hours = seconds // 3600
    minutes = (seconds % 3600) // 60
    secs = seconds % 60
    
    if hours > 0:
        return f"{hours:02d}:{minutes:02d}:{secs:02d}"
    return f"{minutes:02d}:{secs:02d}"


def get_newznab_categories(is_tv: bool, is_hd: bool, resolution: Optional[str] = None, source: Optional[str] = None) -> List[int]:
    """
    Get Newznab category codes based on media type and quality.
    
    Args:
        is_tv: Whether this is TV content
        is_hd: Whether this is HD quality (720p+)
        resolution: Resolution string (e.g., "1080p", "2160p")
        source: Source string (e.g., "BluRay", "WEB-DL")
        
    Returns:
        List of category codes
        
    Newznab Categories:
        TV: 5000-5999
          5030: TV/SD
          5040: TV/HD
        Movies: 2000-2999
          2030: Movies/SD
          2040: Movies/HD
          2050: Movies/BluRay
    """
    categories = []
    
    if is_tv:
        categories.append(5000)  # TV - Main category
        if is_hd:
            categories.append(5040)  # TV/HD
        else:
            categories.append(5030)  # TV/SD
    else:
        # Movies
        categories.append(2000)  # Movies - Main category
        
        # Check for BluRay
        if source and "bluray" in source.lower():
            categories.append(2050)  # Movies/BluRay
        elif is_hd:
            categories.append(2040)  # Movies/HD
        else:
            categories.append(2030)  # Movies/SD
    
    return categories


def format_newznab_attrs(attrs_list: List[Tuple[str, str]]) -> str:
    """
    Format Newznab attributes as XML string.
    
    Args:
        attrs_list: List of (name, value) tuples
        
    Returns:
        XML string with newznab:attr elements
        
    Example:
        >>> attrs = [("size", "12345"), ("video", "x264")]
        >>> format_newznab_attrs(attrs)
        '<newznab:attr name="size" value="12345"/><newznab:attr name="video" value="x264"/>'
    """
    result = []
    for name, value in attrs_list:
        # Escape XML special characters
        value_escaped = value.replace('&', '&amp;').replace('<', '&lt;').replace('>', '&gt;').replace('"', '&quot;')
        result.append(f'<newznab:attr name="{name}" value="{value_escaped}"/>')
    return ''.join(result)
