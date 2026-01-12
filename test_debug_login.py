
import sys
import logging
from pathlib import Path

# Setup path
sys.path.append('/etc/pve/fshare-arr-bridge/src')

from fshare_bridge.core.account_manager import AccountManager
from fshare_bridge.clients.fshare import FshareClient

# Configure logging
logging.basicConfig(level=logging.INFO)
logger = logging.getLogger(__name__)

def test_login():
    print("--- Testing Fshare Login ---")
    
    # 1. Check Account Manager
    am = AccountManager()
    primary = am.get_primary()
    
    if not primary:
        print("❌ No primary account found in AccountManager!")
        return
        
    print(f"Found primary account: {primary.get('email')}")
    
    # 2. Get Client
    client = am.get_primary_client()
    if not client:
        print("❌ Failed to create client from account")
        return
        
    # 3. Try Login
    try:
        print("Attempting login...")
        # We manually call the logic inside login to debug
        
        # Step 1: CSRF
        homepage = client.session.get("https://www.fshare.vn/", timeout=15)
        import re
        csrf_match = re.search(r'name="_csrf-app" value="([^"]+)"', homepage.text)
        if not csrf_match:
            print("❌ Could not find CSRF token on homepage")
            return
        csrf_token = csrf_match.group(1)
        print(f"Got CSRF: {csrf_token[:10]}...")
        
        # Step 2: Post
        response = client.session.post(
            "https://www.fshare.vn/site/login",
            data={
                "_csrf-app": csrf_token,
                "LoginForm[email]": client.email,
                "LoginForm[password]": client.password,
                "LoginForm[rememberMe]": 1
            },
            headers={
                "Referer": "https://www.fshare.vn/",
                "Content-Type": "application/x-www-form-urlencoded",
                "User-Agent": "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36"
            },
            timeout=15
        )
        
        print(f"Response Status: {response.status_code}")
        print(f"Response URL: {response.url}")
        
        if '/site/logout' in response.text:
            print("✅ Login SUCCESS! (Found /site/logout)")
        else:
            print("❌ Login FAILED")
            print("--- HTML Dump (First 1000 chars) ---")
            print(response.text[:1000])
            print("--- Checking for specific errors ---")
            if "Email hoặc mật khẩu không đúng" in response.text:
                print("Found: Incorrect email or password")
            if "recaptcha" in response.text.lower():
                print("Found: RECAPTCHA required!")
            if "Tài khoản đang bị khóa" in response.text:
                print("Found: Account locked")
            
            # Save full html for analysis if needed
            with open("login_fail.html", "w") as f:
                f.write(response.text)
            print("Full response saved to login_fail.html")

    except Exception as e:
        print(f"❌ Exception during login: {e}")

if __name__ == "__main__":
    test_login()
