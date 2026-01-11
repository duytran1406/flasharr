"""
Fshare-Arr Bridge - Filename Parser Module
Normalizes Fshare filenames for *arr suite compatibility
"""

import re
from typing import Optional, Dict, List
from dataclasses import dataclass


@dataclass
class ParsedFilename:
    """Structured representation of parsed filename"""
    original_filename: str
    normalized_filename: str
    title: str
    season: Optional[int] = None
    episode: Optional[int] = None
    year: Optional[int] = None
    quality: Optional[str] = None
    is_series: bool = False


class FilenameNormalizer:
    """
    Normalizes Fshare filenames to be compatible with *arr suite parsing
    
    The key issue: Quality markers appearing BEFORE season/episode markers
    cause *arr to include them in the series title, breaking lookups.
    
    Solution: Move all quality markers AFTER the season/episode marker.
    """
    
    # Quality keywords that should appear AFTER episode marker
    # Quality keywords that should appear AFTER episode marker
    QUALITY_KEYWORDS = [
        # Resolutions
        '2160p', '4K', 'UHD', '8K', '4320p',
        '1080p', '720p', '576p', '480p', '360p',
        
        # Sources
        'BluRay', 'Blu-Ray', 'BDRip', 'BRRip', 'BD25', 'BD50',
        'WEB-DL', 'WEBDL', 'WEBRip', 'WebRip',
        'HDTV', 'PDTV', 'TVRip',
        'DVDRip', 'DVD', 'DVD5', 'DVD9',
        'Remux', 'ISO', 'CAM', 'TS', 'TC',
        
        # Video
        'HDR', 'HDR10', 'HDR10+', 'Dolby Vision', 'DV',
        'SDR', '10Bit', '10bit', '8Bit', '8bit', '12bit',
        'x265', 'x264', 'H.265', 'H.264', 'HEVC', 'AVC',
        'H265', 'H264', 'MPEG-2', 'MPEG-4',
        'XviD', 'DivX', 'VP9', 'AV1',
        
        # Audio
        'AAC', 'AC3', 'DTS', 'DTS-HD', 'DTS-X', 'TrueHD', 'Atmos',
        'DD5.1', 'DD+', 'EAC3', 'FLAC', 'PCM', 'MP3',
        'HE-AAC', 'AAC-LC', 'Vorbis', 'Opus',
        
        # Groups/Tags
        'Proper', 'Repack', 'Real', 'Rerip', 'Hybrid',
        'AMZN', 'NF', 'HULU', 'DSNP',
    ]
    
    # Vietnamese-specific markers
    VIETNAMESE_MARKERS = [
        'TVP', 'TMPĐ', 'Vietsub', 'Thuyết Minh', 'Lồng Tiếng',
        'SP',  # Special episode marker
        'VTV', 'HTV',  # Vietnamese broadcasters
    ]
    
    # Season/Episode patterns
    SE_PATTERNS = [
        r'S(\d{1,4})E(\d{1,3})',  # S01E14
        r'S(\d{1,4})\s*EP?(\d{1,3})',  # S01 E14, S01 EP14
        r'(?<!\w)E(\d{1,3})(?!\w)',  # E14 (standalone)
        r'(?<!\w)EP(\d{1,3})(?!\w)',  # EP14 (standalone)
        r'\s-\s(\d{1,4})(?!\d)', # - 01 (Anime absolute numbering)
    ]
    
    def __init__(self):
        # Compile regex patterns for efficiency
        self.quality_pattern = self._build_quality_pattern()
        # Compile patterns individually
        self.se_patterns = [re.compile(p, re.IGNORECASE) for p in self.SE_PATTERNS]
    
    def _build_quality_pattern(self) -> re.Pattern:
        """Build a regex pattern to match all quality keywords"""
        all_keywords = self.QUALITY_KEYWORDS + self.VIETNAMESE_MARKERS
        # Escape special regex characters and join with |
        escaped = [re.escape(kw) for kw in all_keywords]
        pattern = r'\b(' + '|'.join(escaped) + r')\b'
        return re.compile(pattern, re.IGNORECASE)
    
    def parse(self, filename: str) -> ParsedFilename:
        """
        Parse and normalize filename
        
        Args:
            filename: Original Fshare filename
            
        Returns:
            ParsedFilename object with extracted and normalized data
        """
        # Remove file extension if present
        name, ext = self._split_extension(filename)
        
        # Find season/episode marker
        se_match = None
        matched_pattern_index = -1
        
        for i, pattern in enumerate(self.se_patterns):
            match = pattern.search(name)
            if match:
                se_match = match
                matched_pattern_index = i
                break
        
        if not se_match:
            # No season/episode found, return as-is (movie or unknown)
            return ParsedFilename(
                original_filename=filename,
                normalized_filename=filename,
                title=self._clean_title(name),
                is_series=False
            )
        
        # Extract season and episode
        if matched_pattern_index < 2:
            # Standard SxxExx pattern (index 0 and 1 have 2 groups)
            season = int(se_match.group(1))
            episode = int(se_match.group(2))
        else:
            # Episode only pattern (index 2 and 3 have 1 group)
            season = 1
            episode = int(se_match.group(1))
            
        se_normalized = f"S{season:02d}E{episode:02d}"
        
        # Split into parts: before SE, SE marker, after SE
        before_se = name[:se_match.start()].strip()
        after_se = name[se_match.end():].strip()
        
        # Extract quality markers from the "before" part
        quality_markers = []
        clean_title = before_se
        
        # Find all quality markers in the title part
        for match in self.quality_pattern.finditer(before_se):
            quality_markers.append(match.group(0))
        
        # Remove quality markers from title
        clean_title = self.quality_pattern.sub('', before_se)
        
        # Extract year from title (keep it separate)
        year_match = re.search(r'\b(19\d{2}|20\d{2})\b', clean_title)
        year = int(year_match.group(1)) if year_match else None
        if year:
            clean_title = clean_title.replace(str(year), '')
        
        # Clean up title
        clean_title = re.sub(r'[_\.]', ' ', clean_title)  # Replace _ and . with space
        clean_title = re.sub(r'\s+', ' ', clean_title)  # Normalize spaces
        clean_title = clean_title.strip()
        
        # Clean up the "after" part
        after_se_clean = re.sub(r'[_\.]', ' ', after_se)
        after_se_clean = re.sub(r'\s+', ' ', after_se_clean)
        
        # Remove duplicate year from after part
        if year:
            after_se_clean = re.sub(rf'\(?{year}\)?', '', after_se_clean)
        
        # Reconstruct: Title + SE + Year + Quality + After
        parts = [clean_title, se_normalized]
        
        if year:
            parts.append(str(year))
        
        if quality_markers:
            parts.extend(quality_markers)
        
        if after_se_clean:
            parts.append(after_se_clean)
        
        normalized = ' '.join(parts)
        normalized = re.sub(r'\s+', ' ', normalized).strip()
        
        # Add extension back if it was present
        if ext:
            normalized = f"{normalized}.{ext}"
        
        # Build quality string
        quality_parts = []
        if year:
            quality_parts.append(str(year))
        quality_parts.extend(quality_markers)
        if after_se_clean:
            quality_parts.append(after_se_clean)
        quality_str = ' '.join(quality_parts) if quality_parts else None
        
        return ParsedFilename(
            original_filename=filename,
            normalized_filename=normalized,
            title=clean_title,
            season=season,
            episode=episode,
            year=year,
            quality=quality_str,
            is_series=True
        )
    
    def _split_extension(self, filename: str) -> tuple:
        """Split filename into name and extension"""
        video_extensions = ['mkv', 'mp4', 'avi', 'mov', 'wmv', 'flv', 'webm', 'ts', 'm2ts']
        
        for ext in video_extensions:
            if filename.lower().endswith(f'.{ext}'):
                return filename[:-len(ext)-1], ext
        
        return filename, None
    
    def _clean_title(self, title: str) -> str:
        """Clean up title string"""
        # Replace underscores with spaces FIRST to ensure word boundaries work
        title = title.replace('_', ' ')
        
        # Remove quality and Vietnamese markers
        title = self.quality_pattern.sub('', title)
        
        # Replace remaining dots with spaces
        title = re.sub(r'[\.]', ' ', title)
        
        # Remove year if present
        title = re.sub(r'\b(19\d{2}|20\d{2})\b', '', title)
        # Clean up extra spaces
        title = re.sub(r'\s+', ' ', title)
        return title.strip()
