import requests
import os
import sys
import json
import time
from concurrent.futures import ThreadPoolExecutor, as_completed

# TMDB API Key
TMDB_API_KEY = '8d95150f3391194ca66fef44df497ad6'

sys.path.insert(0, '/etc/pve/fshare-arr-bridge')
from src.flasharr.clients.timfshare import TimFshareClient
from src.flasharr.utils.quality_profile import get_quality_parser
from src.flasharr.utils.normalizer import normalize_filename

client = TimFshareClient()
parser = get_quality_parser()

def get_popular_list(media_type, pages=10):
    all_results = []
    for page in range(1, pages + 1):
        endpoint = f'https://api.themoviedb.org/3/{media_type}/popular'
        resp = requests.get(endpoint, params={'api_key': TMDB_API_KEY, 'language': 'en-US', 'page': page})
        if resp.status_code == 200:
            all_results.extend(resp.json().get('results', []))
        else:
            print(f"Error fetching page {page} for {media_type}: {resp.status_code}")
        time.sleep(0.1)
    return all_results[:200]

def search_item(item, media_type):
    title = item.get('title') if media_type == 'movie' else item.get('name')
    if media_type == 'movie':
        year = item.get('release_date', '')[:4]
        query = f'{title} {year}' if year else title
    else:
        # For TV, pick S01E01 as a standard test point
        query = f'{title} S01E01'
        year = (item.get('first_air_date', '') or '')[:4]

    try:
        results = client.search(query, limit=5, extensions=('.mkv', '.mp4'))
    except Exception as e:
        return {'title': title, 'status': 'Error', 'error': str(e)}

    if not results:
        return {'title': title, 'status': 'Not Found'}

    best = results[0]
    profile = parser.parse(best.name, best.size)
    similarity = client._calculate_similarity(title, best.name)
    
    # TV Specific validation
    se_match = True
    if media_type == 'tv':
        result_s = client._extract_season_from_filename(best.name)
        result_e = client._extract_episode_from_filename(best.name)
        if result_s != 1 or result_e != 1:
            se_match = False

    is_match = similarity >= 0.4 and se_match
    
    return {
        'title': title,
        'year': year,
        'status': 'Found',
        'is_match': is_match,
        'similarity': similarity,
        'quality': profile.quality_name,
        'filename': best.name,
        'se_match': se_match if media_type == 'tv' else None
    }

def run_test():
    print("### STARTING LARGE SCALE TEST (400 ITEMS) ###")
    print("Fetching titles from TMDB...")
    movies = get_popular_list('movie', 10)
    tv_shows = get_popular_list('tv', 10)
    
    results = {'movie': [], 'tv': []}
    
    for mtype in ['movie', 'tv']:
        items = movies if mtype == 'movie' else tv_shows
        print(f"\nSearching {mtype}s (using ThreadPool)...")
        
        with ThreadPoolExecutor(max_workers=5) as executor:
            futures = {executor.submit(search_item, item, mtype): item for item in items}
            count = 0
            for future in as_completed(futures):
                res = future.result()
                results[mtype].append(res)
                count += 1
                if count % 20 == 0:
                    print(f"   Progress: {count}/200")

    # Stats Calculation
    stats = {}
    for mtype in ['movie', 'tv']:
        data = results[mtype]
        found = [r for r in data if r['status'] == 'Found']
        matches = [r for r in found if r['is_match']]
        
        stats[mtype] = {
            'total': len(data),
            'found': len(found),
            'matched': len(matches),
            'failed_se': len([r for r in found if r.get('se_match') == False]) if mtype == 'tv' else 0,
            'qualities': {}
        }
        
        for r in found:
            q = r['quality']
            stats[mtype]['qualities'][q] = stats[mtype]['qualities'].get(q, 0) + 1

    print("\n" + "="*60)
    print("LARGE SCALE TEST RESULTS SUMMARY")
    print("="*60)
    
    for mtype in ['movie', 'tv']:
        s = stats[mtype]
        print(f"\n{mtype.upper()}S:")
        print(f"   Total Queries: {s['total']}")
        print(f"   Fshare Hits:   {s['found']} ({s['found']/s['total']*100:.1f}%)")
        print(f"   Proper Matches: {s['matched']} ({s['matched']/(s['found'] if s['found'] > 0 else 1)*100:.1f}% of hits)")
        if mtype == 'tv':
            print(f"   S/E Mismatch:  {s['failed_se']} (rejected by S1E1 filter)")
        
        print("   Quality Breakdown:")
        sorted_q = sorted(s['qualities'].items(), key=lambda x: -x[1])
        for q, count in sorted_q[:5]:
            print(f"      - {q:15}: {count}")

    # Output details to JSON for reference
    with open('test_results_detailed.json', 'w') as f:
        json.dump(results, f, indent=2)
    print("\nDetailed results saved to test_results_detailed.json")

if __name__ == "__main__":
    run_test()
