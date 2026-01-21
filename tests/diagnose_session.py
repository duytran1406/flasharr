"""
Diagnostic script to understand session validation behavior.
This will help us see why validate_session() returns False after successful login.
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
print("SESSION VALIDATION DIAGNOSTIC")
print("="*60)

# Create client
client = FshareClient(email=TEST_EMAIL, password=TEST_PASSWORD)

print("\n1. Initial state:")
print(f"   is_authenticated: {client.is_authenticated}")
print(f"   _token: {client._token}")
print(f"   _account_type: {client._account_type}")
print(f"   _traffic_left: {client._traffic_left}")

print("\n2. Logging in...")
login_success = client.login()
print(f"   Login result: {login_success}")

print("\n3. State after login:")
print(f"   is_authenticated: {client.is_authenticated}")
print(f"   _token: {client._token}")
print(f"   _account_type: {client._account_type}")
print(f"   _traffic_left: {client._traffic_left}")
print(f"   Cookies: {list(client.session.cookies.keys())}")

print("\n4. Calling validate_session()...")
validation_result = client.validate_session()
print(f"   Validation result: {validation_result}")

print("\n5. State after validation:")
print(f"   is_authenticated: {client.is_authenticated}")
print(f"   _token: {client._token}")
print(f"   _account_type: {client._account_type}")
print(f"   _traffic_left: {client._traffic_left}")

print("\n6. Calling validate_session() again...")
validation_result2 = client.validate_session()
print(f"   Validation result: {validation_result2}")

print("\n" + "="*60)
print("DIAGNOSIS COMPLETE")
print("="*60)

if not validation_result:
    print("\n❌ PROBLEM: validate_session() returned False after successful login!")
    print("   This explains why ensure_authenticated() keeps triggering logins.")
else:
    print("\n✅ validate_session() works correctly")
