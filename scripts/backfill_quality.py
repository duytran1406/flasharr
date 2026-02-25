#!/usr/bin/env python3
"""
Flasharr Quality + Batch Backfill Script
=========================================
Objective:
  1. Parse quality/resolution from the original filename embedded in the
     download URL path (e.g. .../The.Art.of.Sarah.S01E07.1080p.NF.WEB-DL.mkv)
     OR from the Fshare V3 API for items where the URL doesn't contain the
     original filename (raw fshare.vn/file/CODE links).
  2. Update quality + resolution for all downloads (especially 'Unknown-SD').
  3. Update size = 0 rows via Fshare V3 API.
  4. Detect and create batch groups from TMDB-grouped filenames.

Usage:
  # Pull DB from staging, run dry-run first:
  python3 scripts/backfill_quality.py --db /tmp/flasharr_backfill.db --dry-run

  # Apply changes and push back:
  python3 scripts/backfill_quality.py --db /tmp/flasharr_backfill.db --push

Options:
  --db PATH       Path to flasharr.db (required)
  --dry-run       Print planned changes without writing to DB
  --push          After writing, push updated DB back to staging LXC and restart
  --concurrency N Parallel Fshare API requests (default 8)
  --delay N       Seconds between request batches (default 0.3)
  --force-quality Re-parse quality even for rows that already have it set
"""

import argparse
import asyncio
import re
import sqlite3
import sys
import uuid
from pathlib import Path
from collections import defaultdict
from urllib.parse import urlparse

try:
    import aiohttp
except ImportError:
    print("❌  aiohttp required: pip install aiohttp")
    sys.exit(1)

# ─── Quality Parser (mirrors parser.rs) ───────────────────────────────────────

_TS_RE  = re.compile(r'\b(ts|telesync|telecine)\b', re.IGNORECASE)
_TC_RE  = re.compile(r'\b(tc|telecine)\b', re.IGNORECASE)
_DVD_RE = re.compile(r'\b(dvd|dvdrip|dvd5|dvd9)\b', re.IGNORECASE)


def parse_quality(filename: str) -> tuple[str, str | None]:
    """Returns (quality_name, resolution_or_None)"""
    fl = filename.lower()

    # Resolution
    resolution = None
    if '2160p' in fl or '4k' in fl or 'uhd' in fl:
        resolution = '2160p'
    elif '1080p' in fl or '1080i' in fl:
        resolution = '1080p'
    elif '720p' in fl:
        resolution = '720p'
    elif '576p' in fl or '480p' in fl:
        resolution = '480p'

    # Source (word-boundary safe for ts/tc/dvd)
    source = None
    if 'remux' in fl:
        source = 'Remux'
    elif 'bluray' in fl or 'blu-ray' in fl:
        source = 'BluRay'
    elif 'bdrip' in fl or 'brrip' in fl:
        source = 'BDRip'
    elif 'web-dl' in fl or 'webdl' in fl:
        source = 'WebDL'
    elif 'webrip' in fl or 'web-rip' in fl:
        source = 'WEBRip'
    elif 'hdtv' in fl or 'pdtv' in fl:
        source = 'HDTV'
    elif _DVD_RE.search(fl):
        source = 'DVDRip'
    elif _TS_RE.search(fl) or _TC_RE.search(fl):
        source = 'TS'
    elif 'cam' in fl:
        source = 'CAM'

    src = source or 'Unknown'
    if resolution:
        quality_name = f'Remux-{resolution}' if src == 'Remux' else f'{src}-{resolution}'
    else:
        quality_name = 'Unknown' if src == 'Unknown' else f'{src}-Unknown'

    return quality_name, resolution


def extract_original_filename_from_url(url: str) -> str | None:
    """
    For resolved Fshare DL URLs like:
      http://download001.fshare.vn/dl/TOKEN/The.Art.of.Sarah.S01E07.1080p.WEB-DL.mkv
    Extract the filename from the last path segment.
    """
    try:
        path = urlparse(url).path
        last = path.rstrip('/').split('/')[-1]
        # Must look like a video filename (has extension)
        video_exts = ('.mkv', '.mp4', '.avi', '.mov', '.ts', '.m4v', '.wmv', '.webm', '.m2ts', '.flv')
        if any(last.lower().endswith(ext) for ext in video_exts):
            return last
    except Exception:
        pass
    return None


def extract_fshare_code(url: str) -> str | None:
    """Extract fcode from fshare.vn/file/FCODE"""
    if '/file/' in url:
        return url.split('/file/')[-1].split('?')[0].strip() or None
    return None


# ─── Fshare V3 API ────────────────────────────────────────────────────────────

FSHARE_V3 = 'https://www.fshare.vn/api/v3/files/folder'
HEADERS = {
    'Accept': 'application/json, text/plain, */*',
    'User-Agent': 'Mozilla/5.0 (compatible; Flasharr/2.0)',
}


async def fetch_file_info(session: aiohttp.ClientSession, fcode: str) -> dict | None:
    try:
        async with session.get(
            FSHARE_V3, params={'linkcode': fcode},
            headers=HEADERS, timeout=aiohttp.ClientTimeout(total=15),
            ssl=False,
        ) as resp:
            if resp.status != 200:
                print(f'  ⚠  [{fcode}] HTTP {resp.status}')
                return None
            data = await resp.json(content_type=None)
            if data.get('status') == 404:
                print(f'  ⚠  [{fcode}] not found')
                return None
            current = data.get('current', {})
            name = current.get('name', '')
            size = current.get('size', 0)
            if isinstance(size, str):
                size = int(size) if size.isdigit() else 0
            return {'fcode': fcode, 'filename': name, 'size': int(size)}
    except Exception as e:
        print(f'  ✗  [{fcode}] {e}')
        return None


# ─── Batch Detection ──────────────────────────────────────────────────────────

_EP_PATTERNS = [
    re.compile(r'S(\d{1,2})E(\d{1,3})', re.IGNORECASE),
    re.compile(r'[._\-\s]E(\d{1,3})[._\-\s]', re.IGNORECASE),
    re.compile(r'\bEP(\d{1,3})\b', re.IGNORECASE),
]


def filename_pattern_key(filename: str) -> str:
    """Strip episode markers to get a stable pattern key for the series+quality."""
    name = re.sub(r'\.(mkv|mp4|avi|ts|m4v|mov|wmv|flv|webm|m2ts)$', '', filename, flags=re.IGNORECASE)
    name = re.sub(r'S\d{1,2}E\d{1,3}', 'SXXEXX', name, flags=re.IGNORECASE)
    name = re.sub(r'[._\-\s]E\d{1,3}[._\-\s]', '.EXX.', name, flags=re.IGNORECASE)
    name = re.sub(r'\bEP?\d{1,3}\b', 'EXX', name, flags=re.IGNORECASE)
    name = re.sub(r'[._\-\s]\d{1,3}[._\-\s]', '.XX.', name)
    return name.lower()


# ─── Main ─────────────────────────────────────────────────────────────────────

async def main():
    ap = argparse.ArgumentParser()
    ap.add_argument('--db', required=True)
    ap.add_argument('--dry-run', action='store_true')
    ap.add_argument('--push', action='store_true')
    ap.add_argument('--concurrency', type=int, default=8)
    ap.add_argument('--delay', type=float, default=0.3)
    ap.add_argument('--force-quality', action='store_true',
                    help='Re-parse quality even for rows that already have it set')
    args = ap.parse_args()

    db_path = Path(args.db)
    if not db_path.exists():
        print(f'❌  DB not found: {db_path}'); sys.exit(1)

    conn = sqlite3.connect(str(db_path))
    conn.row_factory = sqlite3.Row
    cur = conn.cursor()

    cur.execute("""
        SELECT id, url, original_url, filename, size,
               quality, resolution,
               tmdb_id, tmdb_title, tmdb_season, tmdb_episode,
               batch_id, batch_name
        FROM downloads
        ORDER BY tmdb_id, created_at
    """)
    rows = [dict(r) for r in cur.fetchall()]
    print(f'\n📋  Total downloads: {len(rows)}\n')

    # ── Step 1: Determine the best filename for quality parsing ──────────────
    #   Priority: filename embedded in resolved DL URL > Fshare API > DB filename
    need_api: list[str] = []   # fcodes that need API lookup (size=0 or no filename in URL)

    for row in rows:
        # Try to extract original filename from the resolved DL URL path
        url_fn = extract_original_filename_from_url(row['url'] or '')
        if url_fn:
            row['orig_filename'] = url_fn
        else:
            # No filename in URL — need Fshare API
            orig = row.get('original_url') or ''
            fcode = extract_fshare_code(orig)
            if fcode:
                need_api.append(fcode)
            row['orig_filename'] = None  # will fill from API

        # Also need API if size is 0
        if row['size'] == 0:
            orig = row.get('original_url') or ''
            fcode = extract_fshare_code(orig)
            if fcode and fcode not in need_api:
                need_api.append(fcode)

    unique_fcodes = list(set(need_api))
    api_results: dict[str, dict] = {}

    if unique_fcodes:
        print(f'🌐  Fetching Fshare info for {len(unique_fcodes)} files that need API...\n')
        sem = asyncio.Semaphore(args.concurrency)

        async def bounded(sess, fcode):
            async with sem:
                await asyncio.sleep(args.delay)
                return await fetch_file_info(sess, fcode)

        async with aiohttp.ClientSession() as sess:
            fetched = await asyncio.gather(*[bounded(sess, f) for f in unique_fcodes])

        for info in fetched:
            if info:
                api_results[info['fcode']] = info

        print(f'\n✅  Got info for {len(api_results)}/{len(unique_fcodes)} files\n')

    # Attach API data
    for row in rows:
        if row['orig_filename'] is None:
            orig = row.get('original_url') or ''
            fcode = extract_fshare_code(orig)
            info = api_results.get(fcode) if fcode else None
            if info and info.get('filename'):
                row['orig_filename'] = info['filename']
            else:
                row['orig_filename'] = row['filename']  # last resort: DB name

        # Size from API
        orig = row.get('original_url') or ''
        fcode = extract_fshare_code(orig)
        info = api_results.get(fcode) if fcode else None
        row['api_size'] = info['size'] if info else None

    # ── Step 2: Compute quality updates ─────────────────────────────────────
    quality_updates: list[dict] = []

    for row in rows:
        orig_fn = row['orig_filename'] or row['filename']
        new_quality, new_resolution = parse_quality(orig_fn)
        new_size = row['api_size']

        old_quality = row['quality']
        old_size    = row['size']

        quality_changed = (old_quality != new_quality or row['resolution'] != new_resolution)
        size_needs_fix  = (new_size is not None and old_size == 0 and new_size > 0)

        if (quality_changed and (args.force_quality or old_quality in (None, 'Unknown-SD', 'Unknown'))) or size_needs_fix:
            quality_updates.append({
                'id': row['id'],
                'quality': new_quality,
                'resolution': new_resolution,
                'size': new_size if size_needs_fix else old_size,
                'old_quality': old_quality,
                'old_size': old_size,
                'orig_fn': orig_fn,
            })

    print(f'🎨  Quality/size updates: {len(quality_updates)}')
    for u in quality_updates[:15]:
        q_changed = u['old_quality'] != u['quality']
        s_changed = u['size'] != u['old_size']
        print(f'  [{u["id"][:8]}] {u["orig_fn"][:65]}')
        if q_changed:
            print(f'    quality:    {u["old_quality"]!r:20} → {u["quality"]!r}')
        if s_changed:
            def fmt(b): return f'{b/1e9:.2f}GB' if b > 1e9 else f'{b/1e6:.1f}MB' if b else '0B'
            print(f'    size:       {fmt(u["old_size"])} → {fmt(u["size"])}')
    if len(quality_updates) > 15:
        print(f'  ... and {len(quality_updates)-15} more')

    # ── Step 3: Detect batches ───────────────────────────────────────────────
    print(f'\n🔍  Detecting batches...')
    # Group: downloads sharing (tmdb_id, filename_pattern_key) with >1 item
    groups: dict[tuple, list[dict]] = defaultdict(list)
    for row in rows:
        tmdb_id = row.get('tmdb_id')
        if not tmdb_id:
            continue
        key = (tmdb_id, filename_pattern_key(row['orig_filename'] or row['filename']))
        groups[key].append(row)

    batch_updates: list[dict] = []

    for (tmdb_id, pat), items in groups.items():
        # Only batch if >1 item share the same release pattern
        if len(items) < 2:
            continue

        # Check if already all have the same batch_id
        existing_bids = {r.get('batch_id') for r in items if r.get('batch_id')}
        if len(existing_bids) == 1 and None not in existing_bids:
            print(f'  ⏭  Already batched: tmdb={tmdb_id} ({len(items)} items, bid={next(iter(existing_bids))[:8]})')
            continue

        # Generate batch ID and name
        bid = str(uuid.uuid4())
        sample_fn = items[0]['orig_filename'] or items[0]['filename']
        qname, _ = parse_quality(sample_fn)
        tmdb_title = items[0].get('tmdb_title') or f'TMDB-{tmdb_id}'
        bname = f'{tmdb_title} [{qname}]'

        print(f'  📦  New batch: "{bname}" ({len(items)} items) → {bid[:8]}')
        for item in sorted(items, key=lambda r: r.get('tmdb_episode') or 0):
            batch_updates.append({
                'id': item['id'],
                'batch_id': bid,
                'batch_name': bname,
            })

    print(f'\n📦  Batch assignments: {len(batch_updates)}')

    # ── Step 4: Write ────────────────────────────────────────────────────────
    if args.dry_run:
        print('\n🔵  DRY RUN — no changes written\n')
    else:
        print('\n💾  Writing to DB...')
        for u in quality_updates:
            cur.execute(
                "UPDATE downloads SET quality=?, resolution=?, size=? WHERE id=?",
                (u['quality'], u['resolution'], u['size'], u['id'])
            )
        for b in batch_updates:
            cur.execute(
                "UPDATE downloads SET batch_id=?, batch_name=? WHERE id=?",
                (b['batch_id'], b['batch_name'], b['id'])
            )
        conn.commit()
        print(f'  ✓ {len(quality_updates)} quality/size rows updated')
        print(f'  ✓ {len(batch_updates)} batch assignments written')

    conn.close()

    # Summary
    print(f'\n{"─"*60}')
    print(f'  Quality/size fixes : {len(quality_updates)}')
    print(f'  Batch assignments  : {len(batch_updates)}')
    print(f'{"─"*60}\n')

    # ── Step 5: Push back ────────────────────────────────────────────────────
    if args.push and not args.dry_run:
        import subprocess
        print('🚀  Copying updated DB back to staging LXC 112...')
        # Stop container, replace DB, restart
        cmds = [
            'ssh root@pve-remote "pct exec 112 -- docker stop flasharr"',
            f'ssh root@pve-remote "pct exec 112 -- bash -c \'cat > /var/lib/docker/volumes/flasharr_appdata/_data/data/flasharr.db\'" < {db_path}',
            'ssh root@pve-remote "pct exec 112 -- docker start flasharr"',
        ]
        # Simpler: use docker cp via stdin
        r = subprocess.run(
            f'ssh root@pve-remote "pct exec 112 -- docker cp - flasharr:/appData/data/flasharr.db" < {db_path}',
            shell=True, capture_output=True, text=True
        )
        if r.returncode == 0:
            print('  ✓ DB pushed — restarting container...')
            subprocess.run('ssh root@pve-remote "pct exec 112 -- docker restart flasharr"', shell=True)
            print('  ✓ Container restarted')
        else:
            # Fallback: copy to tmp path inside container then move
            print(f'  ⚠  Direct copy failed ({r.stderr.strip()}), trying cat method...')
            r2 = subprocess.run(
                f'cat {db_path} | ssh root@pve-remote "pct exec 112 -- docker exec -i flasharr bash -c '
                f'\'cat > /tmp/flasharr_new.db && cp /tmp/flasharr_new.db /appData/data/flasharr.db\'"',
                shell=True, capture_output=True, text=True
            )
            if r2.returncode == 0:
                subprocess.run('ssh root@pve-remote "pct exec 112 -- docker restart flasharr"', shell=True)
                print('  ✓ Done')
            else:
                print(f'  ✗ Push failed: {r2.stderr}')
                print(f'\n  Manual push:')
                print(f'    scp {db_path} root@pve-remote:/tmp/')
                print(f'    ssh root@pve-remote "pct exec 112 -- docker cp /tmp/flasharr_backfill.db flasharr:/appData/data/flasharr.db"')
                print(f'    ssh root@pve-remote "pct exec 112 -- docker restart flasharr"')

    print('🎉  Done!\n')


if __name__ == '__main__':
    asyncio.run(main())
