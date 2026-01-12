#!/usr/bin/env python3
"""
Run the Fshare-Arr-Bridge application.
"""

import sys
from pathlib import Path

# Add src to path
src_path = Path(__file__).parent / 'src'
sys.path.insert(0, str(src_path))

from fshare_bridge import run_app

if __name__ == '__main__':
    run_app()
