"""
Enhanced diagnostic to see cookie flow during login.
"""

import sys
sys.path.insert(0, '/etc/pve/fshare-arr-bridge/src')

from flasharr.clients.fshare import FshareClient
import logging

# Enable debug logging
logging.basicConfig(level=logging.DEBUG, format='%(levelname)s: %(message)s')

TEST_EMAIL = "duytran.1406@gmail.com"
TEST_PASSWORD = "duytran1406"

print("="*60)
print("COOKIE FLOW DIAGNOSTIC")
print("="*60)

# Create client
client = FshareClient(email=TEST_EMAIL, password=TEST_PASSWORD)

# Manually trace the login flow
print("\n1. GET /site/login to get CSRF token...")
response1 = client.session.get("https://www.fshare.vn/site/login", timeout=15)
print(f"   Status: {response1.status_code}")
print(f"   Cookies received: {dict(response1.cookies)}")
print(f"   Session cookies after GET: {client.session.cookies.get_dict()}")

# Extract CSRF
import re
csrf_match = re.search(r'name="_csrf-app"\s+value="([^"]+)"', response1.text)
csrf_token = csrf_match.group(1) if csrf_match else None
print(f"   CSRF token: {csrf_token[:50]}..." if csrf_token else "   CSRF token: NOT FOUND")

print("\n2. POST /site/login with credentials...")
print(f"   Cookies being sent: {client.session.cookies.get_dict()}")

response2 = client.session.post(
    "https://www.fshare.vn/site/login",
    data={
        "_csrf-app": csrf_token,
        "LoginForm[email]": TEST_EMAIL,
        "LoginForm[password]": TEST_PASSWORD,
        "LoginForm[rememberMe]": 1
    },
    headers={
        "Referer": "https://www.fshare.vn/site/login",
        "Content-Type": "application/x-www-form-urlencoded",
    },
    timeout=15,
    allow_redirects=True
)

print(f"   Status: {response2.status_code}")
print(f"   Final URL: {response2.url}")
print(f"   Cookies received: {dict(response2.cookies)}")
print(f"   Session cookies after POST: {client.session.cookies.get_dict()}")

print("\n3. GET /account/profile to verify session...")
print(f"   Cookies being sent: {client.session.cookies.get_dict()}")

response3 = client.session.get("https://www.fshare.vn/account/profile", timeout=15)
print(f"   Status: {response3.status_code}")
print(f"   Final URL: {response3.url}")
print(f"   Redirected to login: {'site/login' in response3.url}")

print("\n" + "="*60)
if "site/login" in response3.url:
    print("❌ LOGIN FAILED - Session not established")
    print("\nPossible reasons:")
    print("1. Invalid credentials")
    print("2. Missing required cookies")
    print("3. CSRF token mismatch")
    print("4. Fshare blocking automated logins")
else:
    print("✅ LOGIN SUCCESSFUL - Session established")
print("="*60)
