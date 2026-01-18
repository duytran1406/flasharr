"""
Title Matching Utilities for Smart Search

Provides intelligent title extraction and matching to improve search relevance.
Filters out irrelevant results by comparing core titles and keywords.
"""

import re
import difflib
from typing import Set, List


def extract_core_title(text: str) -> str:
    """
    Extract the core title from a filename or search query.
    
    Removes:
    - Years (1987, 2025, etc.)
    - Quality indicators (1080p, 4K, BluRay, etc.)
    - Release groups (-RARBG, -YTS, etc.)
    - File extensions
    
    Args:
        text: Filename or search query
        
    Returns:
        Cleaned title in lowercase
    """
    # Remove file extension
    name = re.sub(r'\.(mkv|mp4|avi|m4v|wmv|flv|webm|ts|m2ts)$', '', text, flags=re.I)
    
    # Remove year
    name = re.sub(r'\b(19|20)\d{2}\b', '', name)
    
    # Remove quality/source indicators
    quality_patterns = [
        r'\b(1080p|2160p|4K|720p|480p|360p)\b',
        r'\b(BluRay|BRRip|BDRip|WEB-?DL|WEBRip|HDTV|DVDRip|DVD-?Rip)\b',
        r'\b(REMUX|x264|x265|H\.?264|H\.?265|HEVC|AVC)\b',
        r'\b(DTS|AAC|AC3|TrueHD|Atmos|DD5\.1|DD7\.1)\b',
        r'\b(HDR|HDR10|HDR10\+|DV|Dolby\.?Vision|SDR)\b',
        r'\b(PROPER|REPACK|EXTENDED|UNRATED|DIRECTORS?\.?CUT)\b',
    ]
    for pattern in quality_patterns:
        name = re.sub(pattern, '', name, flags=re.I)
    
    # Remove release group (usually at end after dash)
    name = re.sub(r'-[A-Z0-9]+$', '', name, flags=re.I)
    
    # Remove common separators and brackets
    name = re.sub(r'[\[\]()]', ' ', name)
    
    # Replace dots, underscores, dashes with spaces
    name = re.sub(r'[._-]+', ' ', name)
    
    # Remove extra whitespace
    name = ' '.join(name.split())
    
    return name.strip().lower()


def get_title_keywords(title: str) -> Set[str]:
    """
    Extract significant keywords from a title.
    
    Args:
        title: Title string
        
    Returns:
        Set of significant keywords (excluding stop words)
    """
    core = extract_core_title(title)
    
    # Remove common stop words
    stop_words = {
        'the', 'a', 'an', 'and', 'or', 'but', 'in', 'on', 'at', 
        'to', 'for', 'of', 'with', 'by', 'from', 'as', 'is', 'was',
        'are', 'were', 'be', 'been', 'being', 'have', 'has', 'had'
    }
    
    words = set(core.split()) - stop_words
    
    # Filter out very short words (likely not meaningful)
    words = {w for w in words if len(w) > 2}
    
    return words


def calculate_smart_similarity(search_title: str, filename: str) -> float:
    """
    Calculate intelligent similarity between search title and filename.
    
    Uses keyword-based matching to ensure all important words from the
    search title are present in the filename.
    
    Args:
        search_title: The title being searched for
        filename: The filename to compare against
        
    Returns:
        Score from 0.0 to 1.0, where:
        - 1.0 = Perfect match (all keywords, exact order)
        - 0.9 = Excellent match (all keywords present)
        - 0.7-0.8 = Good match (most keywords present)
        - 0.5-0.6 = Partial match (some keywords)
        - < 0.5 = Poor match (few or no keywords)
    """
    search_core = extract_core_title(search_title)
    file_core = extract_core_title(filename)
    
    # Exact match
    if search_core == file_core:
        return 1.0
    
    # Get keywords
    search_words = get_title_keywords(search_title)
    file_words = get_title_keywords(filename)
    
    if not search_words:
        # Fallback to basic similarity if no keywords
        return difflib.SequenceMatcher(None, search_core, file_core).ratio()
    
    # Check keyword overlap
    common_words = search_words & file_words
    
    if not common_words:
        return 0.0  # No common keywords = not a match
    
    # Calculate keyword match ratio
    keyword_match_ratio = len(common_words) / len(search_words)
    
    # All search keywords present in file
    if search_words.issubset(file_words):
        # Calculate how much extra content is in the file
        extra_words = len(file_words - search_words)
        
        if extra_words == 0:
            # Perfect keyword match
            return 0.95
        elif extra_words <= 2:
            # Very close match (e.g., "Predator" vs "Predator Extended")
            return 0.85
        elif extra_words <= 4:
            # Good match but has some extra content
            return 0.75
        else:
            # Has many extra words - might be different movie
            # (e.g., "Predator" vs "Predator Killer Of Killers")
            return 0.60
    
    # Partial keyword match
    # Penalize if file has many extra keywords (likely different movie)
    extra_ratio = len(file_words - search_words) / max(len(file_words), 1)
    penalty = extra_ratio * 0.3
    
    base_score = keyword_match_ratio * 0.7
    
    return max(0.0, base_score - penalty)


def is_likely_different_movie(search_title: str, filename: str, threshold: float = 0.6) -> bool:
    """
    Determine if a filename likely represents a different movie than the search title.
    
    Args:
        search_title: The title being searched for
        filename: The filename to check
        threshold: Similarity threshold (default 0.6)
        
    Returns:
        True if likely a different movie, False otherwise
    """
    similarity = calculate_smart_similarity(search_title, filename)
    return similarity < threshold
