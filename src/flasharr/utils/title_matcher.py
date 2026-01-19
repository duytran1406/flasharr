"""
Title Matching Utilities for Smart Search - Phase 1 Implementation

Provides intelligent title extraction and matching with:
- Unified similarity algorithm
- Franchise conflict detection
- Vietnamese title support (Phase 2 preparation)
- Keyword-based strict matching
"""

import re
import difflib
from typing import Set, List, Dict, Tuple, Optional


# ============================================================================
# FRANCHISE CONFLICT DETECTION
# ============================================================================

FRANCHISE_CONFLICTS = {
    # Format: "franchise_keyword": ["different_movie_indicators"]
    "predator": ["killer of killers", "prey", "dark ages", "alien vs", "requiem"],
    "alien": ["romulus", "resurrection", "covenant", "prometheus", "vs predator"],
    "terminator": ["genisys", "dark fate", "salvation", "rise of the machines"],
    "matrix": ["resurrections", "reloaded", "revolutions"],
    "star wars": ["rogue one", "solo", "mandalorian", "andor", "clone wars"],
    "jurassic": ["world", "dominion", "fallen kingdom"],
    "fast": ["furious", "hobbs", "shaw"],
    "mission impossible": ["dead reckoning", "fallout", "rogue nation", "ghost protocol"],
    "john wick": ["chapter 2", "chapter 3", "chapter 4", "parabellum"],
    "spider-man": ["homecoming", "far from home", "no way home", "into the verse"],
    "avengers": ["ultron", "infinity war", "endgame"],
    "batman": ["begins", "dark knight", "rises", "forever", "robin"],
    "transformers": ["revenge", "dark of the moon", "age of extinction", "last knight"],
    "pirates": ["dead man", "world's end", "stranger tides", "dead men"],
    "harry potter": ["chamber", "prisoner", "goblet", "phoenix", "prince", "hallows"],
    "lord of the rings": ["two towers", "return of the king", "fellowship"],
    "hobbit": ["unexpected", "desolation", "five armies"],
    "hunger games": ["catching fire", "mockingjay", "ballad"],
    "twilight": ["new moon", "eclipse", "breaking dawn"],
}


def is_different_franchise_entry(search_title: str, filename: str) -> bool:
    """
    Detect if filename is a DIFFERENT movie in the same franchise.
    
    Example:
        search: "Predator: Badlands"
        filename: "Predator.Killer.of.Killers.2025.mkv"
        result: True (different movie)
    
    Args:
        search_title: The title being searched for
        filename: The filename to check
        
    Returns:
        True if filename is likely a different franchise entry
    """
    search_lower = search_title.lower()
    file_lower = filename.lower()
    
    for franchise, conflicts in FRANCHISE_CONFLICTS.items():
        # Check if search is for this franchise
        if franchise in search_lower:
            for conflict in conflicts:
                # If conflict term is in filename but NOT in search title
                if conflict in file_lower and conflict not in search_lower:
                    return True
    return False


# ============================================================================
# TITLE EXTRACTION & KEYWORDS
# ============================================================================

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
        r'\b(Vietsub|Thuyet\.?Minh|Long\.?Tieng|Sub\.?Viet)\b',
    ]
    for pattern in quality_patterns:
        name = re.sub(pattern, '', name, flags=re.I)
    
    # Remove release group (usually at end after dash)
    name = re.sub(r'-[A-Z0-9]+$', '', name, flags=re.I)
    
    # Remove common separators and brackets
    name = re.sub(r'[\[\]()]', ' ', name)
    
    # Remove colons and other punctuation (important for titles like "Predator: Badlands")
    name = re.sub(r'[:\'"!?,;]', ' ', name)
    
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
        'are', 'were', 'be', 'been', 'being', 'have', 'has', 'had',
        'i', 'ii', 'iii', 'iv', 'v', 'vi', 'vii', 'viii', 'ix', 'x'  # Roman numerals handled separately
    }
    
    words = set(core.split()) - stop_words
    
    # Filter out very short words (likely not meaningful)
    words = {w for w in words if len(w) > 1}
    
    return words


# ============================================================================
# UNIFIED SIMILARITY ALGORITHM
# ============================================================================

def calculate_unified_similarity(
    search_title: str, 
    filename: str,
    aliases: Optional[List[str]] = None
) -> Dict:
    """
    Unified similarity calculation with alias support.
    
    This is the SINGLE source of truth for similarity matching.
    Replaces the dual-algorithm system.
    
    Args:
        search_title: The primary title being searched for
        filename: The filename to compare against
        aliases: Optional list of alternative titles (e.g., Vietnamese)
        
    Returns:
        Dictionary with:
        - score: float (0.0-1.0)
        - is_valid: bool (True if should be included in results)
        - matched_title: str (which title matched)
        - match_type: str ('exact', 'all_keywords', 'partial', 'alias')
    """
    # Try primary title first
    result = _match_against_title(search_title, filename)
    
    if result['is_valid']:
        result['matched_title'] = search_title
        return result
    
    # If no match and aliases provided, try each alias
    if aliases:
        for alias in aliases:
            alias_result = _match_against_title(alias, filename)
            if alias_result['is_valid']:
                alias_result['matched_title'] = alias
                alias_result['match_type'] = 'alias'
                return alias_result
    
    # No match found
    result['matched_title'] = search_title
    return result


def _match_against_title(search_title: str, filename: str) -> Dict:
    """
    Internal matching function for a single title.
    
    Returns:
        Dictionary with score, is_valid, match_type
    """
    search_core = extract_core_title(search_title)
    file_core = extract_core_title(filename)
    
    # Check for franchise conflicts FIRST
    if is_different_franchise_entry(search_title, filename):
        return {
            'score': 0.0,
            'is_valid': False,
            'match_type': 'franchise_conflict'
        }
    
    # Exact match
    if search_core == file_core:
        return {
            'score': 1.0,
            'is_valid': True,
            'match_type': 'exact'
        }
    
    # Get keywords
    search_words = get_title_keywords(search_title)
    file_words = get_title_keywords(filename)
    
    if not search_words:
        # Fallback to basic similarity if no keywords
        score = difflib.SequenceMatcher(None, search_core, file_core).ratio()
        return {
            'score': score,
            'is_valid': score >= 0.7,
            'match_type': 'fuzzy'
        }
    
    # Calculate keyword overlap
    common_words = search_words & file_words
    missing_words = search_words - file_words
    extra_words = file_words - search_words
    
    # CRITICAL: ALL search keywords must be present
    if missing_words:
        # Partial match - calculate how many are missing
        match_ratio = len(common_words) / len(search_words)
        
        # Strict matching: ALL keywords required (no exceptions)
        # This prevents false positives like "Tâm" matching Naruto episodes
        return {
            'score': match_ratio * 0.5,
            'is_valid': False,
            'match_type': 'missing_keywords'
        }
    
    # All search keywords present - good match!
    # Calculate score based on extra words (fewer extras = better match)
    if len(extra_words) == 0:
        score = 0.95  # Perfect keyword match
    elif len(extra_words) <= 1:
        score = 0.90  # Very close (e.g., "Extended" version)
    elif len(extra_words) <= 2:
        score = 0.85  # Good match
    elif len(extra_words) <= 3:
        score = 0.75  # Acceptable
    else:
        score = 0.65  # Many extra words
    
    return {
        'score': score,
        'is_valid': True,
        'match_type': 'all_keywords'
    }


# ============================================================================
# LEGACY COMPATIBILITY
# ============================================================================

def calculate_smart_similarity(search_title: str, filename: str) -> float:
    """
    Legacy compatibility function.
    
    Use calculate_unified_similarity() for new code.
    This wrapper maintains backward compatibility with existing code.
    """
    result = calculate_unified_similarity(search_title, filename)
    return result['score']


def is_likely_different_movie(search_title: str, filename: str, threshold: float = 0.65) -> bool:
    """
    Determine if a filename likely represents a different movie.
    
    Args:
        search_title: The title being searched for
        filename: The filename to check
        threshold: Score threshold (default 0.65)
        
    Returns:
        True if likely a different movie, False otherwise
    """
    result = calculate_unified_similarity(search_title, filename)
    return not result['is_valid']


# ============================================================================
# VIETNAMESE SUPPORT (Phase 3: Phonetic Matching)
# ============================================================================

VIETNAMESE_CHARS = set('ăâđêôơưáàảãạấầẩẫậắằẳẵặéèẻẽẹếềểễệíìỉĩịóòỏõọốồổỗộớờởỡợúùủũụứừửữựýỳỷỹỵ')

def is_vietnamese_title(text: str) -> bool:
    """
    Detect if text contains Vietnamese characters.
    
    Vietnamese has unique diacritics: ă, â, đ, ê, ô, ơ, ư, and tone marks.
    """
    return any(c in VIETNAMESE_CHARS for c in text.lower())


def normalize_vietnamese(text: str) -> str:
    """
    Normalize Vietnamese text by removing tone marks.
    
    Useful for fuzzy matching: "Bộ Bộ Kinh Tâm" → "bo bo kinh tam"
    """
    viet_to_ascii = {
        'à': 'a', 'á': 'a', 'ả': 'a', 'ã': 'a', 'ạ': 'a',
        'ă': 'a', 'ằ': 'a', 'ắ': 'a', 'ẳ': 'a', 'ẵ': 'a', 'ặ': 'a',
        'â': 'a', 'ầ': 'a', 'ấ': 'a', 'ẩ': 'a', 'ẫ': 'a', 'ậ': 'a',
        'đ': 'd',
        'è': 'e', 'é': 'e', 'ẻ': 'e', 'ẽ': 'e', 'ẹ': 'e',
        'ê': 'e', 'ề': 'e', 'ế': 'e', 'ể': 'e', 'ễ': 'e', 'ệ': 'e',
        'ì': 'i', 'í': 'i', 'ỉ': 'i', 'ĩ': 'i', 'ị': 'i',
        'ò': 'o', 'ó': 'o', 'ỏ': 'o', 'õ': 'o', 'ọ': 'o',
        'ô': 'o', 'ồ': 'o', 'ố': 'o', 'ổ': 'o', 'ỗ': 'o', 'ộ': 'o',
        'ơ': 'o', 'ờ': 'o', 'ớ': 'o', 'ở': 'o', 'ỡ': 'o', 'ợ': 'o',
        'ù': 'u', 'ú': 'u', 'ủ': 'u', 'ũ': 'u', 'ụ': 'u',
        'ư': 'u', 'ừ': 'u', 'ứ': 'u', 'ử': 'u', 'ữ': 'u', 'ự': 'u',
        'ỳ': 'y', 'ý': 'y', 'ỷ': 'y', 'ỹ': 'y', 'ỵ': 'y',
    }
    
    result = text.lower()
    for viet, ascii_char in viet_to_ascii.items():
        result = result.replace(viet, ascii_char)
    return result


def phonetic_match_vietnamese(search: str, filename: str) -> float:
    """
    Match Vietnamese titles that may be romanized differently.
    
    Example: "Bộ Bộ Kinh Tâm" should match:
    - Bo.Bo.Kinh.Tam.mkv
    - BuocBuocKinhTam.mkv
    - Bu.Bu.Jing.Xin.mkv (Mandarin romanization)
    
    Returns similarity score 0.0-1.0
    """
    search_norm = normalize_vietnamese(search)
    file_norm = normalize_vietnamese(filename)
    
    # Remove non-alphanumeric and compare
    search_clean = re.sub(r'[^a-z0-9]', '', search_norm)
    file_clean = re.sub(r'[^a-z0-9]', '', file_norm)
    
    # Exact normalized match
    if search_clean in file_clean:
        return 1.0
    
    # Partial word matching
    search_words = set(search_norm.split())
    file_words = set(file_norm.split())
    
    if search_words and search_words.issubset(file_words):
        return 0.9
    
    # Fuzzy match using difflib
    return difflib.SequenceMatcher(None, search_clean, file_clean).ratio()


# ============================================================================
# CROSS-LANGUAGE ROMANIZATION (Phase 3)
# ============================================================================

# Common romanization mappings for Asian languages
ROMANIZATION_MAP = {
    # Chinese Pinyin variants
    'xin': ['心', 'kinh'],  # heart
    'jing': ['经', 'kinh'],  # classic/through
    'bu': ['步', 'bộ'],  # step
    
    # Korean romanization
    'bo': ['보', 'bộ'],
    'kyung': ['경', 'kinh'],
    'sim': ['심', 'tâm'],
}


def get_romanization_variants(word: str) -> List[str]:
    """
    Get alternative romanizations for a word.
    
    Useful for matching:
    - "Kinh" → "Jing" (Vietnamese → Pinyin)
    - "Tâm" → "Xin" (Vietnamese → Pinyin)
    """
    variants = [word.lower()]
    
    for romanization, equivalents in ROMANIZATION_MAP.items():
        if word.lower() == romanization or word.lower() in equivalents:
            variants.append(romanization)
            variants.extend([e for e in equivalents if isinstance(e, str)])
    
    return list(set(variants))

