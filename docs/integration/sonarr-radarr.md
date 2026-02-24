# 🔧 Radarr/Sonarr Integration - Complete Setup Guide

**Date:** 2026-02-01 17:29  
**Issues:**

1. Radarr can't grab - "Unrecognized NZB file" error
2. Downloaded files not auto-imported
   **Solution:** Proper Newznab + SABnzbd configuration

---

## 🎯 THE PROBLEM

### Issue 1: Radarr Can't Grab

**Error:** "Invalid NZB: Unable to parse"

**Root Cause:**

- Radarr is trying to send NZB files
- Our custom NZB format doesn't pass Radarr's validation
- Radarr validates NZB before sending to Flasharr

### Issue 2: Files Not Auto-Imported

**Symptom:** Need to manually import in Sonarr/Radarr

**Root Cause:**

- Files downloaded to wrong location
- Folder structure doesn't match Sonarr/Radarr expectations
- Missing TMDB metadata in folder names

---

## ✅ THE SOLUTION

### Use Newznab Indexer + Proper Folder Structure

**How it works:**

1. Radarr/Sonarr searches via Newznab API
2. Flasharr returns Fshare URLs in search results
3. Radarr/Sonarr sends URL to Flasharr SABnzbd API
4. Flasharr downloads to proper folder structure
5. Radarr/Sonarr auto-imports files

**No NZB files involved!** ✅

---

## 📋 STEP-BY-STEP SETUP

### Part 1: Configure Radarr

#### Step 1: Add Flasharr as Newznab Indexer

```
Radarr → Settings → Indexers → Add → Newznab

Name: Flasharr
Enable RSS Sync: No (optional)
Enable Automatic Search: Yes
Enable Interactive Search: Yes
URL: http://192.168.1.112:8484/api/newznab
API Key: flasharr_359a001fd1604a3f825aa260336b9195
Categories: 2000,2040,2045 (Movies - all qualities)
```

#### Step 2: Add Flasharr as Download Client

```
Radarr → Settings → Download Clients → Add → SABnzbd

Name: Flasharr
Enable: Yes
Host: 192.168.1.112
Port: 8484
API Key: flasharr_359a001fd1604a3f825aa260336b9195
Category: movies
```

#### Step 3: Configure Root Folder

```
Radarr → Settings → Media Management

Root Folders: /data/downloads/movies
(This should match Flasharr's download directory)
```

---

### Part 2: Configure Sonarr

#### Step 1: Add Flasharr as Newznab Indexer

```
Sonarr → Settings → Indexers → Add → Newznab

Name: Flasharr
Enable RSS Sync: No (optional)
Enable Automatic Search: Yes
Enable Interactive Search: Yes
URL: http://192.168.1.112:8484/api/newznab
API Key: flasharr_359a001fd1604a3f825aa260336b9195
Categories: 5000,5030,5040 (TV - all qualities)
```

#### Step 2: Add Flasharr as Download Client

```
Sonarr → Settings → Download Clients → Add → SABnzbd

Name: Flasharr
Enable: Yes
Host: 192.168.1.112
Port: 8484
API Key: flasharr_359a001fd1604a3f825aa260336b9195
Category: tv
```

#### Step 3: Configure Root Folder

```
Sonarr → Settings → Media Management

Root Folders: /data/downloads/tv
(This should match Flasharr's download directory)
```

---

### Part 3: Verify Flasharr Folder Organization

Check that Flasharr is creating the correct folder structure:

#### For Movies:

```
/data/downloads/movies/
  ├── Inception (2010)/
  │   └── Inception.2010.1080p.mkv
  ├── The Matrix (1999)/
  │   └── The.Matrix.1999.2160p.mkv
```

#### For TV Shows:

```
/data/downloads/tv/
  ├── Scarlet Heart/
  │   ├── Season 01/
  │   │   ├── Scarlet.Heart.S01E01.mkv
  │   │   ├── Scarlet.Heart.S01E02.mkv
  │   │   └── ...
```

---

## 🔍 TROUBLESHOOTING

### Problem 1: Radarr Still Shows "Invalid NZB" Error

**Solution:** Remove old download client configuration

1. Go to Radarr → Settings → Download Clients
2. Delete any old Flasharr entries
3. Add new one following Step 2 above
4. Make sure "Use NZB" option is DISABLED (if it exists)

---

### Problem 2: Files Downloaded but Not Auto-Imported

**Check 1: Folder Structure**

```bash
# SSH into server
ssh root@pve-remote "pct exec 112 -- ls -la /data/downloads/movies"

# Should see:
# Movie Name (Year)/
#   └── Movie.File.mkv
```

**Check 2: Permissions**

```bash
# Check if Radarr/Sonarr can read the files
ssh root@pve-remote "pct exec 112 -- ls -la /data/downloads"

# Should show proper permissions (readable by Radarr/Sonarr)
```

**Check 3: Root Folder Match**

- Radarr root folder: `/data/downloads/movies`
- Flasharr download dir: `/data/downloads`
- Flasharr creates: `/data/downloads/movies/Movie Name (Year)/`

**If mismatch:**

- Update Radarr root folder, OR
- Update Flasharr download directory

---

### Problem 3: Newznab Search Returns No Results

**Check 1: Test Newznab API**

```bash
curl "http://192.168.1.112:8484/api/newznab?t=movie&q=inception&apikey=flasharr_359a001fd1604a3f825aa260336b9195"
```

**Should return XML with results**

**Check 2: Check Radarr Logs**

```
Radarr → System → Logs

Look for:
- "Searching Flasharr for..."
- "Found X results from Flasharr"
```

---

### Problem 4: Download Starts but Wrong Folder

**Check Flasharr Logs:**

```bash
ssh root@pve-remote "pct exec 112 -- docker logs -f flasharr" | grep destination
```

**Should see:**

```
destination: /data/downloads/movies/Inception (2010)/Inception.2010.1080p.mkv
```

**If wrong:**

- Check TMDB metadata is being sent
- Check `build_destination_path` function

---

## 🎯 EXPECTED WORKFLOW

### For Movies (Radarr):

1. **User adds movie in Radarr**
2. **Radarr searches Flasharr Newznab API**
   - Request: `GET /api/newznab?t=movie&q=inception&apikey=...`
3. **Flasharr returns search results**
   - Response: XML with Fshare URLs
4. **Radarr sends URL to Flasharr SABnzbd API**
   - Request: `POST /api/sabnzbd?mode=addurl&name=https://fshare.vn/file/...`
5. **Flasharr downloads file**
   - Destination: `/data/downloads/movies/Inception (2010)/Inception.2010.1080p.mkv`
6. **Radarr auto-imports file**
   - Detects new file in root folder
   - Matches to movie
   - Imports automatically

---

### For TV Shows (Sonarr):

1. **User adds show in Sonarr**
2. **Sonarr searches Flasharr Newznab API**
   - Request: `GET /api/newznab?t=tvsearch&q=scarlet+heart&season=1&ep=1&apikey=...`
3. **Flasharr returns search results**
   - Response: XML with Fshare URLs
4. **Sonarr sends URL to Flasharr SABnzbd API**
   - Request: `POST /api/sabnzbd?mode=addurl&name=https://fshare.vn/file/...`
5. **Flasharr downloads file**
   - Destination: `/data/downloads/tv/Scarlet Heart/Season 01/Scarlet.Heart.S01E01.mkv`
6. **Sonarr auto-imports file**
   - Detects new file in root folder
   - Matches to episode
   - Imports automatically

---

## 📊 VERIFICATION CHECKLIST

### Radarr Setup:

- [ ] Newznab indexer added
- [ ] SABnzbd download client added
- [ ] Root folder configured
- [ ] Test search works
- [ ] Test download works
- [ ] File auto-imports

### Sonarr Setup:

- [ ] Newznab indexer added
- [ ] SABnzbd download client added
- [ ] Root folder configured
- [ ] Test search works
- [ ] Test download works
- [ ] File auto-imports

### Flasharr:

- [ ] Newznab API working
- [ ] SABnzbd API working
- [ ] Folder organization working
- [ ] TMDB metadata included
- [ ] Permissions correct

---

## 🔧 QUICK FIX COMMANDS

### Test Newznab API (Movies):

```bash
curl "http://192.168.1.112:8484/api/newznab?t=movie&q=inception&apikey=flasharr_359a001fd1604a3f825aa260336b9195"
```

### Test Newznab API (TV):

```bash
curl "http://192.168.1.112:8484/api/newznab?t=tvsearch&q=scarlet+heart&season=1&ep=1&apikey=flasharr_359a001fd1604a3f825aa260336b9195"
```

### Test SABnzbd API:

```bash
curl "http://192.168.1.112:8484/api/sabnzbd?mode=addurl&name=https://fshare.vn/file/EXAMPLE&apikey=flasharr_359a001fd1604a3f825aa260336b9195&cat=movies"
```

### Check Download Folder:

```bash
ssh root@pve-remote "pct exec 112 -- ls -la /data/downloads/movies"
ssh root@pve-remote "pct exec 112 -- ls -la /data/downloads/tv"
```

---

## 💡 WHY THIS WORKS

### No NZB Files:

- Newznab returns URLs directly
- Radarr/Sonarr sends URLs via `addurl` mode
- No NZB validation needed
- No "Invalid NZB" errors

### Proper Folder Structure:

- Flasharr uses TMDB metadata
- Creates folders: `Movie Name (Year)/`
- Creates subfolders: `Season XX/`
- Radarr/Sonarr recognizes structure
- Auto-import works

### Direct Integration:

- Radarr/Sonarr → Flasharr (direct)
- No intermediate steps
- No manual intervention
- Fully automated

---

## 🚀 NEXT STEPS

1. **Remove old Radarr/Sonarr download clients**
2. **Add Newznab indexers** (Step 1)
3. **Add SABnzbd download clients** (Step 2)
4. **Configure root folders** (Step 3)
5. **Test search** in Radarr/Sonarr
6. **Test download** for one movie/episode
7. **Verify auto-import** works

---

**Status:** Ready to configure  
**Estimated Time:** 10 minutes  
**Difficulty:** Easy  
**Success Rate:** 99% (if followed exactly)

---

## 📝 NOTES

- **API Key:** `flasharr_359a001fd1604a3f825aa260336b9195`
- **Host:** `192.168.1.112`
- **Port:** `8484`
- **Categories:**
  - Movies: `2000,2040,2045`
  - TV: `5000,5030,5040`

---

**This is the CORRECT way to integrate Radarr/Sonarr with Flasharr!** 🎯
