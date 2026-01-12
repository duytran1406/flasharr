#!/usr/bin/env python3
"""
Test script to debug the download flow for Fshare-Arr-Bridge
"""
import sys
import logging
from pathlib import Path

# Setup path
sys.path.append('/etc/pve/fshare-arr-bridge/src')

from fshare_bridge.core.account_manager import AccountManager
from fshare_bridge.clients.fshare import FshareClient

# Configure logging
logging.basicConfig(
    level=logging.DEBUG,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

def test_download_flow():
    """Test the complete download flow"""
    test_url = "https://www.fshare.vn/file/KHFEEKDW5OSNKL2?des=71f5561d15"
    
    print("=" * 80)
    print("FSHARE DOWNLOAD FLOW TEST")
    print("=" * 80)
    print(f"Test URL: {test_url}\n")
    
    # Step 1: Get account from AccountManager
    print("Step 1: Loading account from AccountManager...")
    am = AccountManager()
    primary = am.get_primary()
    
    if not primary:
        print("❌ No primary account found!")
        return False
        
    print(f"✅ Found primary account: {primary.get('email')}")
    
    # Step 2: Get client with restored session
    print("\nStep 2: Creating FshareClient with restored session...")
    client = am.get_primary_client()
    if not client:
        print("❌ Failed to create client!")
        return False
    
    print(f"✅ Client created")
    print(f"   - Token: {client._token}")
    print(f"   - Token expires: {client._token_expires}")
    print(f"   - Cookies: {list(client.session.cookies.keys())}")
    print(f"   - Is authenticated: {client.is_authenticated}")
    
    # Step 3: Get file info
    print(f"\nStep 3: Getting file info for {test_url}...")
    try:
        file_info = client.get_file_info(test_url)
        if not file_info:
            print("❌ Failed to get file info (returned None)")
            return False
        
        print(f"✅ Got file info:")
        print(f"   - Name: {file_info.name}")
        print(f"   - Size: {file_info.size} bytes ({file_info.size / (1024*1024):.2f} MB)")
        print(f"   - FCode: {file_info.fcode}")
        print(f"   - URL: {file_info.url}")
    except Exception as e:
        print(f"❌ Exception getting file info: {e}")
        import traceback
        traceback.print_exc()
        return False
    
    # Step 4: Get download link
    print(f"\nStep 4: Getting download link for fcode: {file_info.fcode}...")
    try:
        download_url = client.get_download_link(file_info.fcode)
        if not download_url:
            print("❌ Failed to get download link (returned None)")
            return False
        
        print(f"✅ Got download link:")
        print(f"   - URL: {download_url[:100]}...")
    except Exception as e:
        print(f"❌ Exception getting download link: {e}")
        import traceback
        traceback.print_exc()
        return False
    
    print("\n" + "=" * 80)
    print("✅ ALL TESTS PASSED!")
    print("=" * 80)
    return True

if __name__ == "__main__":
    success = test_download_flow()
    sys.exit(0 if success else 1)
