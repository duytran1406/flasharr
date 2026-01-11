"""
Formatting Utilities

Helper functions for formatting sizes, speeds, and durations.
"""

from typing import Union


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
