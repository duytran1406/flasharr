"""
Quality Profile Module

Groups and ranks media files by quality attributes.
Used for deduplication and selection of best version.
"""

import re
from typing import List, Dict, Optional, Tuple
from dataclasses import dataclass, field
from enum import IntEnum


class Resolution(IntEnum):
    """Resolution quality ranking (higher is better)."""
    SD = 1      # 480p, 576p, SD
    HD = 2      # 720p
    FHD = 3     # 1080p
    UHD = 4     # 2160p, 4K


class Source(IntEnum):
    """Source quality ranking (higher is better)."""
    CAM = 1
    TS = 2
    HDTV = 3
    DVDRip = 4
    WEBRip = 5
    WEB_DL = 6
    BDRip = 7
    BluRay = 8
    Remux = 9


class VideoCodec(IntEnum):
    """Video codec preference (higher is better for same resolution)."""
    MPEG2 = 1
    XviD = 2
    H264 = 3   # x264, AVC
    H265 = 4   # x265, HEVC - better compression


class AudioCodec(IntEnum):
    """Audio codec preference (higher is better)."""
    MP3 = 1
    AAC = 2
    AC3 = 3     # DD5.1
    EAC3 = 4    # DD+, DDP
    DTS = 5
    DTS_HD = 6
    TrueHD = 7
    Atmos = 8   # Best spatial audio


@dataclass
class QualityProfile:
    """
    Quality profile for a media file, matching *arr suite model.
    
    In Sonarr/Radarr, Quality = Source + Resolution (e.g., WEBDL-1080p, Bluray-2160p)
    Custom Formats add additional scoring for codecs, audio, HDR, etc.
    """
    resolution: Resolution = Resolution.SD
    source: Source = Source.WEBRip
    video_codec: VideoCodec = VideoCodec.H264
    audio_codec: AudioCodec = AudioCodec.AAC
    hdr: bool = False
    dolby_vision: bool = False
    bit_depth: int = 8
    size_bytes: int = 0
    vietsub: bool = False
    vietdub: bool = False
    
    @property
    def quality_name(self) -> str:
        """
        Get *arr-compatible quality name (Source-Resolution).
        Examples: WEBDL-1080p, Bluray-2160p, HDTV-720p
        """
        source_names = {
            Source.Remux: "Remux",
            Source.BluRay: "Bluray",
            Source.BDRip: "Bluray",  # BDRip maps to Bluray in *arr
            Source.WEB_DL: "WEBDL",
            Source.WEBRip: "WEBRip",
            Source.HDTV: "HDTV",
            Source.DVDRip: "DVD",
            Source.TS: "TS",
            Source.CAM: "CAM",
        }
        
        res_names = {
            Resolution.UHD: "2160p",
            Resolution.FHD: "1080p",
            Resolution.HD: "720p",
            Resolution.SD: "480p",
        }
        
        src = source_names.get(self.source, "Unknown")
        res = res_names.get(self.resolution, "SD")
        
        # Special case for Remux
        if self.source == Source.Remux:
            return f"Remux-{res}"
        
        return f"{src}-{res}"
    
    @property
    def quality_score(self) -> int:
        """
        Calculate base quality score matching Sonarr v5 Weight system.
        Based on: https://github.com/Sonarr/Sonarr/blob/v5-develop/src/NzbDrone.Core/Qualities/Quality.cs
        
        Weight values from DefaultQualityDefinitions:
        - Unknown: 1, SDTV: 2, WEB 480p: 3, DVD: 4, Bluray-480p: 5
        - Bluray-576p: 6, HDTV-720p: 7, HDTV-1080p: 8, Raw-HD: 9
        - WEB 720p: 10, Bluray-720p: 11, WEB 1080p: 12, Bluray-1080p: 13
        - Bluray-1080p Remux: 14, HDTV-2160p: 15, WEB 2160p: 16
        - Bluray-2160p: 17, Bluray-2160p Remux: 18
        """
        # Sonarr v5 weight-based scores (Weight * 10 for finer granularity)
        weight_map = {
            # SD tier
            (Source.CAM, Resolution.SD): 10,      # Unknown
            (Source.TS, Resolution.SD): 15,       # Between Unknown and SDTV
            (Source.HDTV, Resolution.SD): 20,     # SDTV
            (Source.WEBRip, Resolution.SD): 30,   # WEB 480p
            (Source.WEB_DL, Resolution.SD): 30,   # WEB 480p (same group)
            (Source.DVDRip, Resolution.SD): 40,   # DVD
            (Source.BluRay, Resolution.SD): 50,   # Bluray-480p
            
            # HD 720p tier
            (Source.HDTV, Resolution.HD): 70,     # HDTV-720p
            (Source.WEBRip, Resolution.HD): 100,  # WEB 720p
            (Source.WEB_DL, Resolution.HD): 100,  # WEB 720p (same group)
            (Source.BDRip, Resolution.HD): 110,   # Bluray-720p
            (Source.BluRay, Resolution.HD): 110,  # Bluray-720p
            
            # FHD 1080p tier
            (Source.HDTV, Resolution.FHD): 80,    # HDTV-1080p
            (Source.WEBRip, Resolution.FHD): 120, # WEB 1080p
            (Source.WEB_DL, Resolution.FHD): 120, # WEB 1080p (same group)
            (Source.BDRip, Resolution.FHD): 130,  # Bluray-1080p
            (Source.BluRay, Resolution.FHD): 130, # Bluray-1080p
            (Source.Remux, Resolution.FHD): 140,  # Bluray-1080p Remux
            
            # UHD 2160p tier
            (Source.HDTV, Resolution.UHD): 150,   # HDTV-2160p
            (Source.WEBRip, Resolution.UHD): 160, # WEB 2160p
            (Source.WEB_DL, Resolution.UHD): 160, # WEB 2160p (same group)
            (Source.BluRay, Resolution.UHD): 170, # Bluray-2160p
            (Source.Remux, Resolution.UHD): 180,  # Bluray-2160p Remux
        }
        
        return weight_map.get((self.source, self.resolution), 20)
    
    @property
    def custom_format_score(self) -> int:
        """
        Calculate Custom Format score (separate from base quality).
        These are additive bonuses based on *arr Custom Formats.
        """
        score = 0
        
        # HDR bonuses (major preference)
        if self.dolby_vision:
            score += 50  # DV is highly preferred
        elif self.hdr:
            score += 30  # HDR10/HDR10+
            
        # Language bonuses (High priority for Vietnamese content)
        if self.vietdub:
            score += 100 # Priority 1: Vietdub/Thuyet Minh
        elif self.vietsub:
            score += 10  # Priority 2: Vietsub
        
        # Codec efficiency bonus
        if self.video_codec == VideoCodec.H265:
            score += 10  # x265/HEVC preferred for efficiency
        
        # Audio quality bonus
        if self.audio_codec >= AudioCodec.Atmos:
            score += 15
        elif self.audio_codec >= AudioCodec.TrueHD:
            score += 12
        elif self.audio_codec >= AudioCodec.DTS_HD:
            score += 10
        elif self.audio_codec >= AudioCodec.EAC3:
            score += 5
        
        # 10-bit color bonus
        if self.bit_depth >= 10:
            score += 5
        
        return score
    
    @property
    def total_score(self) -> int:
        """Total score = base quality + custom format bonuses."""
        return self.quality_score + self.custom_format_score
    
    def to_dict(self) -> Dict:
        return {
            "quality_name": self.quality_name,
            "resolution": self.resolution.name,
            "source": self.source.name,
            "video_codec": self.video_codec.name,
            "audio_codec": self.audio_codec.name,
            "hdr": self.hdr,
            "dolby_vision": self.dolby_vision,
            "vietsub": self.vietsub,
            "vietdub": self.vietdub,
            "quality_score": self.quality_score,
            "custom_format_score": self.custom_format_score,
            "total_score": self.total_score,
        }


class QualityParser:
    """Parses quality attributes from filenames."""
    
    # Resolution patterns
    RESOLUTION_MAP = {
        Resolution.UHD: [r'2160p', r'4k', r'uhd', r'4320p', r'8k'],
        Resolution.FHD: [r'1080p', r'1080i'],
        Resolution.HD: [r'720p'],
        Resolution.SD: [r'480p', r'576p', r'sd', r'dvd'],
    }
    
    # Source patterns
    SOURCE_MAP = {
        Source.Remux: [r'remux'],
        Source.BluRay: [r'blu-?ray', r'bd25', r'bd50'],
        Source.BDRip: [r'bdrip', r'brrip'],
        Source.WEB_DL: [r'web-?dl', r'webdl'],
        Source.WEBRip: [r'webrip'],
        Source.HDTV: [r'hdtv', r'pdtv'],
        Source.DVDRip: [r'dvdrip', r'dvd'],
        Source.TS: [r'\bts\b', r'\btc\b'],
        Source.CAM: [r'\bcam\b', r'hdcam'],
    }
    
    # Video codec patterns
    VIDEO_CODEC_MAP = {
        VideoCodec.H265: [r'x265', r'hevc', r'h\.?265'],
        VideoCodec.H264: [r'x264', r'avc', r'h\.?264'],
        VideoCodec.XviD: [r'xvid', r'divx'],
        VideoCodec.MPEG2: [r'mpeg-?2'],
    }
    
    # Audio codec patterns
    AUDIO_CODEC_MAP = {
        AudioCodec.Atmos: [r'atmos'],
        AudioCodec.TrueHD: [r'truehd'],
        AudioCodec.DTS_HD: [r'dts-?hd', r'dts-?x', r'dts:x'],
        AudioCodec.DTS: [r'\bdts\b'],
        AudioCodec.EAC3: [r'dd\+', r'ddp', r'eac3', r'ddp5\.?1'],
        AudioCodec.AC3: [r'\bac3\b', r'dd5\.?1', r'dolby digital'],
        AudioCodec.AAC: [r'\baac\b'],
        AudioCodec.MP3: [r'\bmp3\b'],
    }
    
    # Language patterns
    VIETSUB_PATTERNS = [r'vietsub', r'viet sub', r'sub viet', r'sub\.viet', r'phu de', r'phụ đề']
    VIETDUB_PATTERNS = [r'thuyet minh', r'thuyết minh', r'long tieng', r'lồng tiếng', r'vietdub', r'viet dub']
    
    def parse(self, filename: str, size_bytes: int = 0) -> QualityProfile:
        """Parse quality profile from filename."""
        fn_lower = filename.lower()
        profile = QualityProfile(size_bytes=size_bytes)
        
        # Parse resolution
        for res, patterns in self.RESOLUTION_MAP.items():
            if any(re.search(p, fn_lower) for p in patterns):
                profile.resolution = res
                break
        
        # Parse source
        for src, patterns in self.SOURCE_MAP.items():
            if any(re.search(p, fn_lower) for p in patterns):
                profile.source = src
                break
        
        # Parse video codec
        for codec, patterns in self.VIDEO_CODEC_MAP.items():
            if any(re.search(p, fn_lower) for p in patterns):
                profile.video_codec = codec
                break
        
        # Parse audio codec
        for codec, patterns in self.AUDIO_CODEC_MAP.items():
            if any(re.search(p, fn_lower) for p in patterns):
                profile.audio_codec = codec
                break
        
        # Parse HDR
        if re.search(r'hdr10\+|hdr10|hdr', fn_lower):
            profile.hdr = True
        
        # Parse Dolby Vision
        if re.search(r'dolby.?vision|\bdv\b|\.dv\.', fn_lower):
            profile.dolby_vision = True
            profile.hdr = True
        
        # Parse bit depth
        if re.search(r'10.?bit', fn_lower):
            profile.bit_depth = 10
        elif re.search(r'12.?bit', fn_lower):
            profile.bit_depth = 12
            
        # Parse Language Tags
        if any(re.search(p, fn_lower) for p in self.VIETSUB_PATTERNS):
            profile.vietsub = True
            
        if any(re.search(p, fn_lower) for p in self.VIETDUB_PATTERNS):
            profile.vietdub = True
        
        return profile


def group_by_quality(results: List[Dict], parser: QualityParser = None) -> Dict[str, List[Dict]]:
    """
    Group search results by *arr quality name.
    
    Returns dict with keys like 'WEBDL-2160p', 'WEBDL-1080p', 'Bluray-1080p', etc.
    Each value is a list of results sorted by total score.
    """
    if parser is None:
        parser = QualityParser()
    
    groups = {}
    
    for result in results:
        filename = result.get('name', '')
        size = result.get('size', result.get('size_bytes', 0))
        
        profile = parser.parse(filename, size)
        quality_name = profile.quality_name
        
        # Add quality info to result
        result['quality_profile'] = profile.to_dict()
        result['quality_name'] = quality_name
        result['quality_score'] = profile.quality_score
        result['custom_format_score'] = profile.custom_format_score
        result['total_score'] = profile.total_score
        
        if quality_name not in groups:
            groups[quality_name] = []
        groups[quality_name].append(result)
    
    # Sort each group by quality score
    for tier in groups:
        groups[tier].sort(key=lambda x: x.get('quality_score', 0), reverse=True)
    
    return groups


def select_best_per_episode(results: List[Dict], parser: QualityParser = None) -> Dict[str, Dict]:
    """
    Select the best quality version for each episode.
    
    Returns dict keyed by episode identifier (e.g., 'S01E01').
    Uses total_score (base quality + custom format) for ranking.
    """
    if parser is None:
        parser = QualityParser()
    
    best = {}
    
    for result in results:
        filename = result.get('name', '')
        size = result.get('size', result.get('size_bytes', 0))
        
        # Extract episode identifier
        match = re.search(r'S(\d{1,2})E(\d{1,3})', filename, re.IGNORECASE)
        if not match:
            continue
        
        ep_id = f"S{int(match.group(1)):02d}E{int(match.group(2)):02d}"
        
        profile = parser.parse(filename, size)
        result['quality_profile'] = profile.to_dict()
        result['quality_name'] = profile.quality_name
        result['quality_score'] = profile.quality_score
        result['custom_format_score'] = profile.custom_format_score
        result['total_score'] = profile.total_score
        
        # Keep best version by total_score
        if ep_id not in best or result['total_score'] > best[ep_id].get('total_score', 0):
            best[ep_id] = result
    
    return best


# Singleton
_parser = None

def get_quality_parser() -> QualityParser:
    global _parser
    if not _parser:
        _parser = QualityParser()
    return _parser
