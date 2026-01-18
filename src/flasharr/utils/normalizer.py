"""
Filename Normalizer Module

Converts Fshare filenames to *arr-compatible format (Newznab/Sonarr/Radarr).
Ensures consistent naming that *arr applications can parse correctly.
"""

import re
from typing import Optional
from dataclasses import dataclass


@dataclass
class NormalizedResult:
    """Represents a normalized filename ready for *arr consumption."""
    original_filename: str
    normalized_filename: str
    title: str
    year: Optional[int] = None
    season: Optional[int] = None
    episode: Optional[int] = None
    quality: str = ""
    release_group: str = ""
    is_series: bool = False
    
    def to_newznab_title(self) -> str:
        """Generate Newznab-compatible title string."""
        if self.is_series:
            # TV format: Title S01E01 Quality-Group
            base = f"{self.title} S{self.season:02d}E{self.episode:02d}"
        else:
            # Movie format: Title (Year) Quality-Group
            base = f"{self.title}"
            if self.year:
                base = f"{self.title} ({self.year})"
        
        if self.quality:
            base = f"{base} {self.quality}"
        
        if self.release_group:
            base = f"{base}-{self.release_group}"
        
        return base


class FilenameNormalizer:
    """
    Normalizes Fshare filenames to *arr-compatible format.
    
    Fshare filenames often have:
    - Vietnamese markers mixed in (VTV, TMPĐ, Vietsub)
    - Non-standard separators
    - Quality markers in wrong positions
    - Missing or malformed S/E markers
    
    This normalizer restructures them to:
    - Title SxxExx Quality-Group.ext (TV)
    - Title (Year) Quality-Group.ext (Movie)
    """
    
    # Quality markers to extract (order matters for priority)
    QUALITY_MARKERS = [
        # Resolution
        "2160p", "4K", "UHD", "1080p", "720p", "480p", "576p",
        # Source
        "BluRay", "Blu-Ray", "BDRip", "BRRip", "Remux",
        "WEB-DL", "WEBDL", "WEBRip",
        "HDTV", "DVDRip", "DVD",
        # HDR
        "HDR10+", "HDR10", "HDR", "DV", "Dolby Vision",
        # Codec
        "x265", "HEVC", "H.265", "x264", "AVC", "H.264",
        # Audio
        "DTS-HD", "DTS-X", "DTS", "TrueHD", "Atmos",
        "DD5.1", "DD+", "EAC3", "AC3", "AAC", "FLAC",
        "DDP5.1", "DDP", "5.1", "7.1",
        # Streaming services (should be removed from title)
        "NF", "AMZN", "HULU", "DSNP", "ATVP", "PMTP", "HMAX",
        "HBO", "MAX", "PCOK", "STAN", "iT", "CBS", "VIU",
        # Release tags
        "PROPER", "REPACK", "RERIP", "INTERNAL", "EXTENDED",
        "UNRATED", "DIRECTORS", "CUT", "HYBRID",
        # Container (sometimes in name)
        "MKV", "MP4", "AVI",
    ]
    
    # Vietnamese markers to remove from normalized output
    VIET_MARKERS = [
        "VTV", "HTV", "TMPĐ", "TVP", "SP",
        "Vietsub", "VietSub", "Thuyet Minh", "Thuyết Minh",
        "Long Tieng", "Lồng Tiếng", "Phu De", "Phụ Đề",
        "vie.dub", "vie.sub",
    ]
    
    # Video extensions
    VIDEO_EXTENSIONS = (".mkv", ".mp4", ".avi", ".mov", ".wmv", ".ts", ".m2ts")
    
    # S/E patterns (priority order)
    SE_PATTERNS = [
        (re.compile(r'S(\d{1,2})E(\d{1,3})', re.IGNORECASE), True),  # S01E01
        (re.compile(r'S(\d{1,2})\s*EP?(\d{1,3})', re.IGNORECASE), True),  # S01 E01
        (re.compile(r'(\d{1,2})x(\d{1,3})', re.IGNORECASE), True),  # 1x01
        (re.compile(r'\s-\s(\d{2,4})(?!\d)', re.IGNORECASE), False),  # Anime - 01
    ]
    
    # Year pattern
    YEAR_PATTERN = re.compile(r'\b(19[5-9]\d|20[0-2]\d)\b')
    
    # Release group pattern (at end of filename before extension)
    GROUP_PATTERN = re.compile(r'-([A-Za-z0-9]+)(?:\.[a-z0-9]+)?$', re.IGNORECASE)
    
    def normalize(self, filename: str) -> NormalizedResult:
        """
        Normalize a filename to *arr-compatible format.
        
        Args:
            filename: Original Fshare filename
            
        Returns:
            NormalizedResult with normalized data
        """
        # Remove extension
        name, ext = self._split_extension(filename)
        
        # Extract release group first (before other processing)
        release_group = self._extract_release_group(name)
        if release_group:
            # Remove group from name for further processing
            name = re.sub(rf'-{re.escape(release_group)}$', '', name, flags=re.IGNORECASE)
        
        # Check for S/E pattern
        season, episode, se_match = self._extract_se(name)
        
        # Extract year
        year = self._extract_year(name)
        
        # Extract quality markers
        quality_parts = self._extract_quality(name)
        
        # Clean title (remove quality, year, S/E, Vietnamese markers)
        title = self._extract_title(name, se_match, year)
        
        # Build normalized filename
        if season is not None and episode is not None:
            # TV Show
            normalized = self._build_tv_filename(title, season, episode, quality_parts, release_group, ext)
            is_series = True
        else:
            # Movie
            normalized = self._build_movie_filename(title, year, quality_parts, release_group, ext)
            is_series = False
        
        return NormalizedResult(
            original_filename=filename,
            normalized_filename=normalized,
            title=title,
            year=year,
            season=season,
            episode=episode,
            quality=" ".join(quality_parts) if quality_parts else "",
            release_group=release_group or "",
            is_series=is_series,
        )
    
    def _split_extension(self, filename: str) -> tuple:
        """Split filename into name and extension."""
        lower = filename.lower()
        for ext in self.VIDEO_EXTENSIONS:
            if lower.endswith(ext):
                return filename[:-len(ext)], ext[1:]  # Remove leading dot
        return filename, ""
    
    def _extract_release_group(self, name: str) -> Optional[str]:
        """Extract release group from end of filename."""
        match = self.GROUP_PATTERN.search(name)
        if match:
            group = match.group(1)
            # Filter out common false positives
            if group.lower() not in ("mkv", "mp4", "avi", "720p", "1080p", "2160p"):
                return group
        return None
    
    def _extract_se(self, name: str) -> tuple:
        """Extract season and episode numbers."""
        for pattern, has_both in self.SE_PATTERNS:
            match = pattern.search(name)
            if match:
                if has_both:
                    return int(match.group(1)), int(match.group(2)), match
                else:
                    # Anime format: season 1, episode from match
                    return 1, int(match.group(1)), match
        return None, None, None
    
    def _extract_year(self, name: str) -> Optional[int]:
        """Extract year from filename."""
        match = self.YEAR_PATTERN.search(name)
        return int(match.group(1)) if match else None
    
    def _extract_quality(self, name: str) -> list:
        """Extract quality markers from filename."""
        found = []
        name_lower = name.lower()
        
        for marker in self.QUALITY_MARKERS:
            if marker.lower() in name_lower:
                found.append(marker)
        
        return found
    
    def _extract_title(self, name: str, se_match, year: Optional[int]) -> str:
        """
        Extract clean title from filename.
        
        Strategy:
        1. Remove parenthesized prefixes (TM - Vietsub), [Long tieng], etc.
        2. For movies: use year as the end delimiter
        3. For TV: use S/E marker as the end delimiter
        4. Clean technical markers and normalize
        """
        title = name
        
        # Step 1: Remove leading parenthesized/bracketed content
        # Matches: (TM - Vietsub), [Long tieng - Vietsub], etc.
        title = re.sub(r'^[\[\(][^\]\)]*[\]\)]\s*', '', title)
        
        # Step 2: For TV shows, title is BEFORE the S/E marker
        if se_match:
            title = title[:se_match.start()]
        # Step 3: For movies, title is BEFORE the year
        elif year:
            year_match = re.search(rf'\b{year}\b', title)
            if year_match:
                title = title[:year_match.start()]
        
        # Step 4: Remove Vietnamese audio/sub markers
        viet_patterns = [
            r'\bViE[-.]?DUB\b', r'\bViE[-.]?SUB\b', r'\bViE\b',
            r'\bTM\b', r'\bLT\b', r'\bVS\b',  # Common abbreviations
            r'\bDUB\b', r'\bSUB\b',
        ]
        for pattern in viet_patterns:
            title = re.sub(pattern, '', title, flags=re.IGNORECASE)
        
        # Step 5: Remove Vietnamese markers
        for marker in self.VIET_MARKERS:
            title = re.sub(rf'\b{re.escape(marker)}\b', '', title, flags=re.IGNORECASE)
        
        # Step 6: Remove quality markers
        for marker in self.QUALITY_MARKERS:
            title = re.sub(rf'\b{re.escape(marker)}\b', '', title, flags=re.IGNORECASE)
        
        # Step 7: Remove movie number patterns (e.g., "Movie 43", "the Movie 43")
        title = re.sub(r'\b(the\s+)?Movie\s+\d+\b', 'the Movie', title, flags=re.IGNORECASE)
        
        # Step 8: Remove standalone numbers that are likely not part of title
        # But keep numbers that are part of words like "2001" (if not year) or "K-19"
        title = re.sub(r'\s+\d{1,2}\s+', ' ', title)  # Remove standalone 1-2 digit numbers
        
        # Step 9: Clean up separators and whitespace
        title = re.sub(r'[._]', ' ', title)
        title = re.sub(r'\s+', ' ', title)
        title = re.sub(r'[^\w\s\'-]', '', title)  # Keep only letters, numbers, ', -
        title = re.sub(r'\s*-\s*$', '', title)  # Remove trailing dash
        title = title.strip()
        
        # Step 10: Title case
        title = title.title()
        
        # Step 11: Fix common title case issues (small words should be lowercase)
        small_words = ['A', 'An', 'The', 'And', 'But', 'Or', 'For', 'Nor', 'On', 'At', 'To', 'By', 'Of']
        words = title.split()
        if len(words) > 1:
            for i, word in enumerate(words[1:], 1):  # Skip first word
                if word in small_words:
                    words[i] = word.lower()
            title = ' '.join(words)
        
        return title
    
    def _build_tv_filename(self, title: str, season: int, episode: int, 
                           quality: list, group: Optional[str], ext: str) -> str:
        """Build normalized TV filename."""
        parts = [title, f"S{season:02d}E{episode:02d}"]
        
        if quality:
            parts.extend(quality[:3])  # Limit to 3 quality markers
        
        name = " ".join(parts)
        
        if group:
            name = f"{name}-{group}"
        
        if ext:
            name = f"{name}.{ext}"
        
        return name
    
    def _build_movie_filename(self, title: str, year: Optional[int],
                              quality: list, group: Optional[str], ext: str) -> str:
        """Build normalized movie filename."""
        if year:
            base = f"{title} ({year})"
        else:
            base = title
        
        parts = [base]
        
        if quality:
            parts.extend(quality[:3])  # Limit to 3 quality markers
        
        name = " ".join(parts)
        
        if group:
            name = f"{name}-{group}"
        
        if ext:
            name = f"{name}.{ext}"
        
        return name


# Singleton instance
_normalizer = None

def get_normalizer() -> FilenameNormalizer:
    """Get singleton normalizer instance."""
    global _normalizer
    if not _normalizer:
        _normalizer = FilenameNormalizer()
    return _normalizer


def normalize_filename(filename: str) -> NormalizedResult:
    """Convenience function to normalize a filename."""
    return get_normalizer().normalize(filename)
