#!/usr/bin/env python3
"""
Quick test for quality attribute extraction
"""

import sys
sys.path.insert(0, '/etc/pve/fshare-arr-bridge/src')

from fshare_bridge.utils.filename_parser import FilenameParser

def test_quality_extraction():
    parser = FilenameParser()
    
    test_cases = [
        "Ling Cage S01E14 2019 4K HDR 10Bit SP TVP TMPĐ 2160p",
        "Breaking Bad S05E16 1080p BluRay x264",
        "The Mandalorian S02E08 1080p WEB-DL AAC",
        "Movie Name 2024 720p HDTV x265",
    ]
    
    print("=" * 80)
    print("QUALITY ATTRIBUTE EXTRACTION TEST")
    print("=" * 80)
    
    for filename in test_cases:
        print(f"\n📁 Filename: {filename}")
        
        parsed = parser.parse(filename)
        
        if parsed.quality_attrs:
            attrs = parsed.quality_attrs
            print(f"   Resolution: {attrs.resolution}")
            print(f"   Source: {attrs.source}")
            print(f"   Video: {attrs.video_codec}")
            print(f"   Audio: {attrs.audio_codec}")
            print(f"   HDR: {attrs.hdr}")
            print(f"   is_HD: {attrs.is_hd}")
            print(f"   is_TV: {attrs.is_tv}")
            
            # Newznab attributes
            newznab_attrs = attrs.to_newznab_attrs()
            if newznab_attrs:
                print(f"   Newznab: {newznab_attrs}")
        print("-" * 80)

if __name__ == "__main__":
    test_quality_extraction()
