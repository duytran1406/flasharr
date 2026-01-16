
import sys
import os
import logging
from pathlib import Path
import time

# Add src to path
sys.path.append(str(Path(__file__).parent.parent / "src"))

# Configure logging
logging.basicConfig(level=logging.INFO, format='%(levelname)s: %(message)s')
logger = logging.getLogger("test_download")

try:
    from flasharr.core.account_manager import AccountManager
    from flasharr.clients.fshare import FshareClient
    from flasharr.core.config import FshareConfig
except ImportError as e:
    print(f"Import Error: {e}")
    sys.exit(1)

TEST_URLS = [
    "https://www.fshare.vn/file/3ZFTK5Q1NWCNZRJ?des=71f5561d15",
    "https://www.fshare.vn/file/Y94K6TFC4C33YAA?des=71f5561d15"
]

def verify_download_flow():
    # 1. Load Account
    mgr = AccountManager(storage_path=Path("data/accounts.json"))
    if not mgr.accounts:
        print("❌ No accounts found.")
        return

    acc = mgr.accounts[0]
    email = acc.get('email')
    print(f"--- Loading Account: {email} ---")
    
    cookies = acc.get('cookies')
    if not cookies:
        print("❌ No cookies found in storage. Cannot test cookie-based flow.")
        return

    # 2. Init Client & Inject Cookies
    config = FshareConfig(email=email, password=acc.get('password', 'dummy'))
    client = FshareClient.from_config(config)
    
    print("Injecting cookies...")
    client.set_cookies(cookies)
    
    # TRICK: We intentionally DO NOT set a valid token or expiration initially
    # to rely PURELY on cookies. However, `is_authenticated` logic checks for token OR cookies.
    # We want to ensure it DOES NOT call login().
    
    # We can monkeypatch login to fail if called, to prove we didn't use it.
    original_login = client.login
    def fail_login():
        print("❌ ERROR: Client attempted to LOGIN! Cookies should have been sufficient.")
        # return original_login() # Uncomment to allow fallback if you want to see if it works with login
        return False
    client.login = fail_login

    print("--- Starting Download Tests (Login Disabled) ---")
    
    for url in TEST_URLS:
        print(f"\nTarget: {url}")
        try:
            # Get Direct Link
            start_t = time.time()
            direct_link = client.get_download_link(url)
            duration = time.time() - start_t
            
            if direct_link:
                print(f"✅ Success! Generated Link ({duration:.2f}s)")
                print(f"   Link: {direct_link}")
                
                # Verify link is alive with HEAD
                try:
                    head = client.session.head(direct_link, allow_redirects=True, timeout=10)
                    if head.status_code == 200:
                        size = head.headers.get('Content-Length', 'Unknown')
                        print(f"   alive: HTTP 200 (Size: {size} bytes)")
                    else:
                        print(f"   warning: Link returned HTTP {head.status_code}")
                except Exception as e:
                     print(f"   warning: Could not verify link HEAD: {e}")
                     
            else:
                print("❌ Failed to generate link.")
                
        except Exception as e:
            print(f"❌ Exception during test: {e}")
            import traceback
            traceback.print_exc()

if __name__ == "__main__":
    verify_download_flow()
