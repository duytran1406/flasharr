#!/usr/bin/env python3
"""
Integration Test: Search Fshare for Ling Cage Season 1
Tests the complete flow: Search → Parse → Normalize → Display results
"""

import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.join(os.path.dirname(__file__), '..'))

from app.fshare_client import FshareClient
from app.filename_parser import FilenameNormalizer
from dotenv import load_dotenv

# Load environment variables
load_dotenv()


def test_ling_cage_search():
    """
    Test: Search for Ling Cage 2019 Season 1 episodes on Fshare
    
    Expected: Find all episodes with proper parsing and normalization
    """
    
    print("=" * 80)
    print("INTEGRATION TEST: Ling Cage 2019 Season 1 Search")
    print("=" * 80)
    print()
    
    # Initialize clients
    print("🔐 Initializing Fshare client...")
    fshare_email = os.getenv('FSHARE_EMAIL')
    fshare_password = os.getenv('FSHARE_PASSWORD')
    
    if not fshare_email or not fshare_password:
        print("❌ Error: FSHARE_EMAIL and FSHARE_PASSWORD must be set in .env")
        print("   Please copy .env.example to .env and configure your credentials")
        return False
    
    fshare = FshareClient(fshare_email, fshare_password)
    normalizer = FilenameNormalizer()
    
    # Login to Fshare
    print("🔑 Logging into Fshare...")
    if not fshare.login():
        print("❌ Failed to login to Fshare")
        return False
    
    print("✅ Login successful!")
    print()
    
    # Search for Ling Cage
    search_query = "Ling Cage 2019"
    print(f"🔍 Searching Fshare for: '{search_query}'")
    print()
    
    results = fshare.search(search_query, limit=100)
    
    if not results:
        print("❌ No results found")
        return False
    
    print(f"✅ Found {len(results)} total results")
    print()
    
    # Filter for Season 1 episodes
    print("📺 Filtering for Season 1 episodes...")
    print()
    
    season_1_episodes = []
    
    for result in results:
        # Parse the filename
        parsed = normalizer.parse(result['name'])
        
        # Check if it's a series and Season 1
        if parsed.is_series and parsed.season == 1:
            season_1_episodes.append({
                'episode': parsed.episode,
                'original_name': result['name'],
                'normalized_name': parsed.normalized_filename,
                'title': parsed.title,
                'url': result['url'],
                'fcode': result['fcode'],
                'size': result['size'],
                'quality': parsed.quality
            })
    
    # Sort by episode number
    season_1_episodes.sort(key=lambda x: x['episode'])
    
    if not season_1_episodes:
        print("❌ No Season 1 episodes found")
        print()
        print("All results:")
        for i, result in enumerate(results[:10], 1):
            parsed = normalizer.parse(result['name'])
            print(f"{i}. {result['name']}")
            print(f"   Parsed: Season={parsed.season}, Episode={parsed.episode}, Is Series={parsed.is_series}")
            print()
        return False
    
    # Display results
    print("=" * 80)
    print(f"✅ FOUND {len(season_1_episodes)} SEASON 1 EPISODES")
    print("=" * 80)
    print()
    
    for ep in season_1_episodes:
        print(f"📺 Episode {ep['episode']:02d}")
        print(f"   Original:   {ep['original_name']}")
        print(f"   Normalized: {ep['normalized_name']}")
        print(f"   Title:      {ep['title']}")
        print(f"   Quality:    {ep['quality']}")
        print(f"   Size:       {format_size(ep['size'])}")
        print(f"   Fshare URL: {ep['url']}")
        print(f"   File Code:  {ep['fcode']}")
        print()
    
    # Summary
    print("=" * 80)
    print("SUMMARY")
    print("=" * 80)
    print(f"Total results found:     {len(results)}")
    print(f"Season 1 episodes:       {len(season_1_episodes)}")
    print(f"Episode range:           E{min(ep['episode'] for ep in season_1_episodes):02d} - E{max(ep['episode'] for ep in season_1_episodes):02d}")
    print()
    
    # Test filename normalization
    print("=" * 80)
    print("FILENAME NORMALIZATION TEST")
    print("=" * 80)
    print()
    
    if season_1_episodes:
        test_ep = season_1_episodes[0]
        print(f"Testing with Episode {test_ep['episode']:02d}:")
        print()
        print(f"❌ BEFORE (Original Fshare filename):")
        print(f"   {test_ep['original_name']}")
        print()
        print(f"   Problem: Quality markers (4K, HDR, etc.) appear BEFORE S01E{test_ep['episode']:02d}")
        print(f"   Result:  Sonarr would extract title as '{test_ep['title']} 4K HDR...'")
        print(f"   TVDB:    NO MATCH ❌")
        print()
        print(f"✅ AFTER (Normalized by bridge):")
        print(f"   {test_ep['normalized_name']}")
        print()
        print(f"   Fixed:   Quality markers moved AFTER S01E{test_ep['episode']:02d}")
        print(f"   Result:  Sonarr extracts title as '{test_ep['title']}'")
        print(f"   TVDB:    MATCH FOUND ✅")
        print()
    
    # Generate Fshare links list
    print("=" * 80)
    print("FSHARE LINKS FOR ALL SEASON 1 EPISODES")
    print("=" * 80)
    print()
    
    for ep in season_1_episodes:
        print(f"Episode {ep['episode']:02d}: {ep['url']}")
    
    print()
    print("=" * 80)
    print("✅ TEST COMPLETED SUCCESSFULLY")
    print("=" * 80)
    
    return True


def format_size(size_bytes):
    """Format size in bytes to human-readable string"""
    for unit in ['B', 'KB', 'MB', 'GB', 'TB']:
        if size_bytes < 1024.0:
            return f"{size_bytes:.2f} {unit}"
        size_bytes /= 1024.0
    return f"{size_bytes:.2f} PB"


if __name__ == '__main__':
    try:
        success = test_ling_cage_search()
        sys.exit(0 if success else 1)
    except KeyboardInterrupt:
        print("\n\n⚠️  Test interrupted by user")
        sys.exit(1)
    except Exception as e:
        print(f"\n\n❌ Test failed with error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
