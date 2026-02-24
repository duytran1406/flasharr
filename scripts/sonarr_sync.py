#!/usr/bin/env python3
import requests
import sys
import os
import sqlite3
import json
import re

# Configuration
SONARR_URL = os.environ.get("SONARR_URL", "http://192.168.1.110:8989")
SONARR_API_KEY = os.environ.get("SONARR_API_KEY", "6cc69aeeec80439eadc8ec63ef63f839")
DB_PATH = os.environ.get("DB_PATH", "/bulk-storage/appdata/flasharr/data/flasharr.db")
IMPORT_BASE = "/bulk-storage/data/downloads/Sonarr-Import"

# Path mapping for host vs container
PATH_MAP = {
    "/downloads/": "/bulk-storage/data/downloads/",
    "/appData/": "/bulk-storage/appdata/flasharr/"
}

def map_path(path):
    for container_prefix, host_prefix in PATH_MAP.items():
        if path.startswith(container_prefix):
            return path.replace(container_prefix, host_prefix, 1)
    return path

def parse_se_from_filename(filename):
    match = re.search(r'[Ss](\d+)[Ee](\d+)', filename)
    if match:
        return int(match.group(1)), int(match.group(2))
    return None, None

def get_completed_downloads(series_name):
    conn = sqlite3.connect(DB_PATH)
    cursor = conn.cursor()
    query = """
    SELECT filename, destination, tmdb_title, tmdb_season, tmdb_episode 
    FROM downloads 
    WHERE state = 'COMPLETED' 
    AND (filename LIKE ? OR tmdb_title LIKE ?)
    """
    pattern = f"%{series_name}%"
    cursor.execute(query, (pattern, pattern))
    results = cursor.fetchall()
    conn.close()
    return results

def trigger_command(name, payload_extra=None):
    url = f"{SONARR_URL.rstrip('/')}/api/v3/command"
    headers = {"X-Api-Key": SONARR_API_KEY}
    payload = {"name": name}
    if payload_extra:
        payload.update(payload_extra)
    try:
        response = requests.post(url, headers=headers, json=payload)
        response.raise_for_status()
        print(f"  [ok] Triggered Sonarr command: {name}")
    except Exception as e:
        print(f"  [!] Failed to trigger Sonarr command {name}: {e}")

def get_series_info(name):
    url = f"{SONARR_URL.rstrip('/')}/api/v3/series"
    headers = {"X-Api-Key": SONARR_API_KEY}
    try:
        response = requests.get(url, headers=headers)
        response.raise_for_status()
        for series in response.json():
            if name.lower() in series['title'].lower():
                return series['id'], series['title']
    except: pass
    return None, None

if __name__ == "__main__":
    series_query = "Scarlet Heart"
    if len(sys.argv) > 1:
        series_query = sys.argv[1]
    
    s_id, s_title = get_series_info(series_query)
    if not s_id:
        print(f"[!] Could not find series '{series_query}' in Sonarr")
        sys.exit(1)
        
    print(f"[*] Syncing: {s_title} (ID: {s_id})")
    
    downloads = get_completed_downloads(series_query)
    if not downloads:
        print(f"[!] No completed downloads found.")
        sys.exit(0)
        
    # Create a dedicated import folder for this series
    import_dir = os.path.join(IMPORT_BASE, s_title)
    os.makedirs(import_dir, exist_ok=True)
    
    print(f"[*] Import directory: {import_dir}")
    
    for filename, dest, title, season, episode in downloads:
        if season is None or episode is None:
            s, e = parse_se_from_filename(filename)
            if s is not None: season, episode = s, e
        
        if season is None or episode is None: continue
        
        host_src = map_path(dest)
        extension = os.path.splitext(host_src)[1]
        standard_name = f"{s_title} - S{season:02d}E{episode:02d}{extension}"
        host_dst = os.path.join(import_dir, standard_name)
        
        if not os.path.exists(host_src):
            print(f"  [!] Source not found: {host_src}")
            continue
            
        if not os.path.exists(host_dst):
            try:
                os.link(host_src, host_dst)
                print(f"  [+] Linked: {standard_name}")
            except Exception as e:
                print(f"  [!] Link failed: {e}")
        else:
            print(f"  [.] Exists: {standard_name}")
            
    # Trigger scan
    sonarr_import_path = import_dir.replace("/bulk-storage/data/", "/data/", 1)
    trigger_command("DownloadedEpisodesScan", {"path": sonarr_import_path})
    trigger_command("RescanSeries", {"seriesId": s_id})
    
    print("[*] Sync complete.")
