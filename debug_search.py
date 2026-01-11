import requests
import json

def debug_doraemon():
    url = "https://timfshare.com/api/v1/string-query-search?query=doraemon"
    headers = {
        'content-length': '0',
        'origin': 'https://timfshare.com',
        'referer': 'https://timfshare.com/search?key=doraemon',
        'user-agent': 'Mozilla/5.0'
    }
    
    print(f"Querying: {url}")
    try:
        response = requests.post(url, headers=headers, timeout=15)
        print(f"Status: {response.status_code}")
        
        if response.status_code == 200:
            data = response.json().get('data', [])
            print(f"Total raw results: {len(data)}")
            
            # Print extensions
            extensions = {}
            valid_videos = 0
            VIDEO_EXTENSIONS = ('.mp4', '.avi', '.mov', '.mkv', '.m4v', '.flv', '.mpeg', '.wav')
            
            print("\nFirst 20 results:")
            for i, item in enumerate(data[:20]):
                name = item.get('name', 'Unknown')
                print(f"{i+1}. {name}")
                
                ext = "No Extension"
                if "." in name:
                    ext = name.split(".")[-1].lower()
                
                extensions[ext] = extensions.get(ext, 0) + 1
                
                if name.lower().endswith(VIDEO_EXTENSIONS):
                    valid_videos += 1
            
            print(f"\nTotal Valid Videos (in full list): {sum(1 for x in data if x.get('name', '').lower().endswith(VIDEO_EXTENSIONS))}")
            
            # Check autocomplete behavior
            print("\nChecking Autocomplete...")
            ac_url = "https://timfshare.com/api/v1/autocomplete?query=doraemon"
            ac_resp = requests.get(ac_url, headers=headers)
            if ac_resp.status_code == 200:
                ac_data = ac_resp.json().get('data', [])
                if ac_data:
                    print(f"Autocomplete top suggestion: {ac_data[0].get('value')}")
            
    except Exception as e:
        print(f"Error: {e}")

if __name__ == "__main__":
    debug_doraemon()
