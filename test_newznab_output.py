#!/usr/bin/env python3
"""
Test quality attributes in Newznab XML output
"""

import sys
sys.path.insert(0, '/etc/pve/fshare-arr-bridge/src')

from fshare_bridge.services.indexer import IndexerService
from fshare_bridge.clients.timfshare import SearchResult

def test_newznab_output():
    # Create indexer service
    indexer = IndexerService()
    
    # Create test results
    test_results = [
        SearchResult(
            name="Ling Cage S01E14 2019 4K HDR 10Bit SP TVP TMPĐ 2160p.mkv",
            url="https://www.fshare.vn/file/TEST123",
            size=2147483648,
            fcode="TEST123",
            score=90
        ),
        SearchResult(
            name="Breaking Bad S05E16 1080p BluRay x264.mkv",
            url="https://www.fshare.vn/file/TEST456",
            size=1073741824,
            fcode="TEST456",
            score=85
        ),
    ]
    
    # Build response
    response = indexer._build_search_response(test_results)
    
    print("=" * 80)
    print("NEWZNAB XML OUTPUT TEST")
    print("=" * 80)
    print(response.xml)
    print("=" * 80)
    print(f"\nItems count: {response.items_count}")
    
    # Check for quality attributes
    xml_str = response.xml
    checks = [
        ("Category 5000", "5000" in xml_str),
        ("Category 5040 (TV HD)", "5040" in xml_str),
        ("Video codec", 'name="video"' in xml_str),
        ("Season attribute", 'name="season"' in xml_str),
        ("Episode attribute", 'name="episode"' in xml_str),
        ("Relevance score", "Relevance:" in xml_str),
    ]
    
    print("\n✅ Quality Attribute Checks:")
    for check_name, result in checks:
        status = "✅" if result else "❌"
        print(f"   {status} {check_name}: {result}")

if __name__ == "__main__":
    test_newznab_output()
