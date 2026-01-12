"""
Filename Parser Module

Normalizes Fshare filenames for *arr suite compatibility.
Refactored with improved typing and structure.
"""

import re
from typing import Optional, List, Tuple, Set
from dataclasses import dataclass, field


@dataclass
class QualityAttributes:
    """Quality attributes for Newznab API reporting."""
    
    # Resolution/Source
    resolution: Optional[str] = None  # 2160p, 1080p, 720p, 480p, SD
    source: Optional[str] = None      # BluRay, WEB-DL, HDTV, DVDRip, etc.
    
    # Codecs
    video_codec: Optional[str] = None  # x264, x265, HEVC, AVC, etc.
    audio_codec: Optional[str] = None  # AAC, AC3, DTS, etc.
    
    # HDR/Color
    hdr: bool = False
    dolby_vision: bool = False
    
    # Size (from file if available)
    size_bytes: Optional[int] = None
    
    # Newznab categories
    is_tv: bool = False
    is_movie: bool = False
    is_hd: bool = False  # HD quality (720p or higher)
    
    def to_newznab_attrs(self) -> List[Tuple[str, str]]:
        """Convert to Newznab attribute name/value pairs."""
        attrs = []
        
        if self.size_bytes:
            attrs.append(("size", str(self.size_bytes)))
        
        if self.video_codec:
            attrs.append(("video", self.video_codec))
        
        if self.audio_codec:
            attrs.append(("audio", self.audio_codec))
        
        return attrs
    
    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        return {
            "resolution": self.resolution,
            "source": self.source,
            "video_codec": self.video_codec,
            "audio_codec": self.audio_codec,
            "hdr": self.hdr,
            "dolby_vision": self.dolby_vision,
            "is_hd": self.is_hd,
        }


@dataclass
class ParsedFilename:
    """Structured representation of a parsed media filename."""
    
    original_filename: str
    normalized_filename: str
    title: str
    season: Optional[int] = None
    episode: Optional[int] = None
    year: Optional[int] = None
    quality: Optional[str] = None
    is_series: bool = False
    quality_attrs: Optional[QualityAttributes] = None
    
    def to_dict(self) -> dict:
        """Convert to dictionary for JSON serialization."""
        result = {
            "original_filename": self.original_filename,
            "normalized_filename": self.normalized_filename,
            "title": self.title,
            "season": self.season,
            "episode": self.episode,
            "year": self.year,
            "quality": self.quality,
            "is_series": self.is_series,
        }
        
        if self.quality_attrs:
            result["quality_attrs"] = self.quality_attrs.to_dict()
        
        return result


@dataclass
class ParserConfig:
    """Configuration for filename parsing."""
    
    # Quality keywords that should appear AFTER episode marker
    quality_keywords: Tuple[str, ...] = (
        # Resolutions
        "2160p", "4K", "UHD", "8K", "4320p",
        "1080p", "720p", "576p", "480p", "360p",
        # Sources
        "BluRay", "Blu-Ray", "BDRip", "BRRip", "BD25", "BD50",
        "WEB-DL", "WEBDL", "WEBRip", "WebRip",
        "HDTV", "PDTV", "TVRip",
        "DVDRip", "DVD", "DVD5", "DVD9",
        "Remux", "ISO", "CAM", "TS", "TC",
        # Video codecs
        "HDR", "HDR10", "HDR10+", "Dolby Vision", "DV",
        "SDR", "10Bit", "10bit", "8Bit", "8bit", "12bit",
        "x265", "x264", "H.265", "H.264", "HEVC", "AVC",
        "H265", "H264", "MPEG-2", "MPEG-4",
        "XviD", "DivX", "VP9", "AV1",
        # Audio codecs
        "AAC", "AC3", "DTS", "DTS-HD", "DTS-X", "TrueHD", "Atmos",
        "DD5.1", "DD+", "EAC3", "FLAC", "PCM", "MP3",
        "HE-AAC", "AAC-LC", "Vorbis", "Opus",
        # Release tags
        "Proper", "Repack", "Real", "Rerip", "Hybrid",
        "AMZN", "NF", "HULU", "DSNP",
    )
    
    # Vietnamese-specific markers
    vietnamese_markers: Tuple[str, ...] = (
        "TVP", "TMPĐ", "Vietsub", "Thuyết Minh", "Lồng Tiếng",
        "SP", "VTV", "HTV",
    )
    
    # Video file extensions
    video_extensions: Tuple[str, ...] = (
        "mkv", "mp4", "avi", "mov", "wmv", "flv", "webm", "ts", "m2ts",
    )
    
    # Season/Episode regex patterns
    se_patterns: Tuple[str, ...] = (
        r"S(\d{1,4})E(\d{1,3})",           # S01E14
        r"S(\d{1,4})\s*EP?(\d{1,3})",      # S01 E14, S01 EP14
        r"(?<!\w)E(\d{1,3})(?!\w)",        # E14 (standalone)
        r"(?<!\w)EP(\d{1,3})(?!\w)",       # EP14 (standalone)
        r"\s-\s(\d{1,4})(?!\d)",           # - 01 (Anime absolute numbering)
    )


class FilenameParser:
    """
    Parses and normalizes media filenames for *arr suite compatibility.
    
    The key issue: Quality markers appearing BEFORE season/episode markers
    cause *arr to include them in the series title, breaking lookups.
    
    Solution: Move all quality markers AFTER the season/episode marker.
    
    Example:
        >>> parser = FilenameParser()
        >>> result = parser.parse("Show.Name.1080p.S01E05.BluRay.mkv")
        >>> print(result.normalized_filename)
        'Show Name S01E05 1080p BluRay.mkv'
    """
    
    def __init__(self, config: Optional[ParserConfig] = None):
        """
        Initialize the parser.
        
        Args:
            config: Optional custom configuration
        """
        self.config = config or ParserConfig()
        
        # Build compiled patterns
        self._quality_pattern = self._build_quality_pattern()
        self._se_patterns = [
            re.compile(p, re.IGNORECASE) 
            for p in self.config.se_patterns
        ]
        self._year_pattern = re.compile(r"\b(19\d{2}|20\d{2})\b")
    
    def _build_quality_pattern(self) -> re.Pattern:
        """Build regex pattern for quality keyword matching."""
        all_keywords = list(self.config.quality_keywords) + list(self.config.vietnamese_markers)
        escaped = [re.escape(kw) for kw in all_keywords]
        pattern = r"\b(" + "|".join(escaped) + r")\b"
        return re.compile(pattern, re.IGNORECASE)
    
    def parse(self, filename: str) -> ParsedFilename:
        """
        Parse and normalize a media filename.
        
        Args:
            filename: Original filename
            
        Returns:
            ParsedFilename with extracted and normalized data
        """
        # Split extension
        name, ext = self._split_extension(filename)
        
        # Find season/episode marker
        se_match, pattern_index = self._find_se_marker(name)
        
        if not se_match:
            # No S/E found - treat as movie
            quality_attrs = self.extract_quality_attributes(filename)
            quality_attrs.is_movie = True
            
            return ParsedFilename(
                original_filename=filename,
                normalized_filename=filename,
                title=self._clean_title(name),
                year=self._extract_year(name),
                is_series=False,
                quality_attrs=quality_attrs,
            )
        
        # Extract season and episode numbers
        season, episode = self._extract_se_numbers(se_match, pattern_index)
        se_normalized = f"S{season:02d}E{episode:02d}"
        
        # Split into before/after S/E marker
        before_se = name[:se_match.start()].strip()
        after_se = name[se_match.end():].strip()
        
        # Extract quality markers from title part
        quality_markers = self._quality_pattern.findall(before_se)
        clean_title = self._quality_pattern.sub("", before_se)
        
        # Extract and remove year
        year = self._extract_year(clean_title)
        if year:
            clean_title = clean_title.replace(str(year), "")
        
        # Clean up title
        clean_title = self._normalize_text(clean_title)
        
        # Clean up after part
        after_clean = self._normalize_text(after_se)
        if year:
            after_clean = re.sub(rf"\(?{year}\)?", "", after_clean).strip()
        
        # Reconstruct normalized filename
        parts = [clean_title, se_normalized]
        if year:
            parts.append(str(year))
        parts.extend(quality_markers)
        if after_clean:
            parts.append(after_clean)
        
        normalized = " ".join(parts)
        normalized = re.sub(r"\s+", " ", normalized).strip()
        
        if ext:
            normalized = f"{normalized}.{ext}"
        
        # Build quality string
        quality_parts = []
        if year:
            quality_parts.append(str(year))
        quality_parts.extend(quality_markers)
        if after_clean:
            quality_parts.append(after_clean)
        quality_str = " ".join(quality_parts) if quality_parts else None
        
        # Extract quality attributes
        quality_attrs = self.extract_quality_attributes(normalized)
        quality_attrs.is_tv = True
        
        return ParsedFilename(
            original_filename=filename,
            normalized_filename=normalized,
            title=clean_title,
            season=season,
            episode=episode,
            year=year,
            quality=quality_str,
            is_series=True,
            quality_attrs=quality_attrs,
        )
    
    def _find_se_marker(self, name: str) -> Tuple[Optional[re.Match], int]:
        """Find season/episode marker in filename."""
        for i, pattern in enumerate(self._se_patterns):
            match = pattern.search(name)
            if match:
                return match, i
        return None, -1
    
    def _extract_se_numbers(self, match: re.Match, pattern_index: int) -> Tuple[int, int]:
        """Extract season and episode numbers from match."""
        if pattern_index < 2:
            # Standard SxxExx pattern (2 groups)
            return int(match.group(1)), int(match.group(2))
        else:
            # Episode only pattern (1 group)
            return 1, int(match.group(1))
    
    def _extract_year(self, text: str) -> Optional[int]:
        """Extract year from text."""
        match = self._year_pattern.search(text)
        return int(match.group(1)) if match else None
    
    def _split_extension(self, filename: str) -> Tuple[str, Optional[str]]:
        """Split filename into name and extension."""
        lower = filename.lower()
        for ext in self.config.video_extensions:
            if lower.endswith(f".{ext}"):
                return filename[:-len(ext)-1], ext
        return filename, None
    
    def _normalize_text(self, text: str) -> str:
        """Normalize text by replacing separators and cleaning whitespace."""
        text = re.sub(r"[_.]", " ", text)
        text = re.sub(r"\s+", " ", text)
        return text.strip()
    
    def _clean_title(self, title: str) -> str:
        """Clean up title string for movies."""
        title = title.replace("_", " ")
        title = self._quality_pattern.sub("", title)
        title = re.sub(r"[.]", " ", title)
        title = self._year_pattern.sub("", title)
        title = re.sub(r"\s+", " ", title)
        return title.strip()
    
    def extract_quality_attributes(self, filename: str) -> QualityAttributes:
        """
        Extract quality attributes from filename for Newznab reporting.
        
        Args:
            filename: Original or normalized filename
            
        Returns:
            QualityAttributes with extracted quality information
        """
        filename_lower = filename.lower()
        attrs = QualityAttributes()
        
        # Resolution detection
        if "2160p" in filename_lower or "4k" in filename_lower or "uhd" in filename_lower:
            attrs.resolution = "2160p"
            attrs.is_hd = True
        elif "1080p" in filename_lower:
            attrs.resolution = "1080p"
            attrs.is_hd = True
        elif "720p" in filename_lower:
            attrs.resolution = "720p"
            attrs.is_hd = True
        elif "480p" in filename_lower or "576p" in filename_lower:
            attrs.resolution = "480p"
        elif "sd" in filename_lower or "dvd" in filename_lower:
            attrs.resolution = "SD"
        
        # Source detection (priority order)
        if "remux" in filename_lower or "blu-ray" in filename_lower or "bluray" in filename_lower:
            attrs.source = "BluRay"
        elif "web-dl" in filename_lower or "webdl" in filename_lower:
            attrs.source = "WEB-DL"
        elif "webrip" in filename_lower:
            attrs.source = "WEBRip"
        elif "hdtv" in filename_lower:
            attrs.source = "HDTV"
        elif "bdrip" in filename_lower or "brrip" in filename_lower:
            attrs.source = "BDRip"
        elif "dvdrip" in filename_lower:
            attrs.source = "DVDRip"
        elif "cam" in filename_lower:
            attrs.source = "CAM"
        elif "ts" in filename_lower:
            attrs.source = "TS"
        
        # Video codec detection
        if "x265" in filename_lower or "hevc" in filename_lower or "h.265" in filename_lower or "h265" in filename_lower:
            attrs.video_codec = "x265"
        elif "x264" in filename_lower or "avc" in filename_lower or "h.264" in filename_lower or "h264" in filename_lower:
            attrs.video_codec = "x264"
        elif "xvid" in filename_lower:
            attrs.video_codec = "XviD"
        elif "av1" in filename_lower:
            attrs.video_codec = "AV1"
        elif "vp9" in filename_lower:
            attrs.video_codec = "VP9"
        
        # Infer codec from resolution + bit depth if not explicitly stated
        if not attrs.video_codec:
            if "10bit" in filename_lower or "10-bit" in filename_lower:
                if attrs.resolution in ["2160p", "1080p"]:
                    attrs.video_codec = "x265"  # 10-bit usually indicates HEVC
        
        # Audio codec detection
        if "dts-hd" in filename_lower or "dts-x" in filename_lower:
            attrs.audio_codec = "DTS-HD"
        elif "dts" in filename_lower:
            attrs.audio_codec = "DTS"
        elif "truehd" in filename_lower or "atmos" in filename_lower:
            attrs.audio_codec = "TrueHD"
        elif "dd+" in filename_lower or "eac3" in filename_lower:
            attrs.audio_codec = "EAC3"
        elif "ac3" in filename_lower or "dd5.1" in filename_lower:
            attrs.audio_codec = "AC3"
        elif "aac" in filename_lower:
            attrs.audio_codec = "AAC"
        elif "flac" in filename_lower:
            attrs.audio_codec = "FLAC"
        
        # HDR detection
        if "hdr10+" in filename_lower:
            attrs.hdr = True
        elif "hdr10" in filename_lower or "hdr" in filename_lower:
            attrs.hdr = True
        
        if "dolby vision" in filename_lower or "dv" in filename_lower:
            attrs.dolby_vision = True
            attrs.hdr = True  # DV implies HDR
        
        return attrs


# Convenience alias for backwards compatibility
FilenameNormalizer = FilenameParser
