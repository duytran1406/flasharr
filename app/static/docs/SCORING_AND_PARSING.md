# Scoring and Parsing Logic Documentation

This document details how the Fshare Nexus application parses filenames and calculates quality scores for search results.

## 1. Filename Parsing

To ensure clean and readable search results, raw Fshare filenames are processed through `filename_parser.py`.

### Normalization Process
1. **Extension Splitting**: The file extension (e.g., .mkv, .mp4) is managed separately.
2. **Title Extraction**:
    - The parser identifies Season/Episode markers (e.g., `S01E05`) to split the filename.
    - Everything before the marker becomes the candidate title.
    - If no marker is found, the entire name is treated as the title.
3. **Cleaning Steps**:
    - **Quality Stripping**: Keywords like `4K`, `1080p`, `BluRay` are removed from the title to avoid clutter (e.g., "Avengers.Age.of.Ultron.2015.4K" -> "Avengers Age of Ultron 2015").
    - **Language Stripping**: Vietnamese markers like `Vietsub`, `Thuyết Minh` are removed.
    - **Formatting**: Dots (`.`) and underscores (`_`) are replaced with spaces.

## 2. Score System (Accuracy First)
Search results are scored from **0 to 100**, heavily prioritizing how well the filename matches your search query.

### Formula
**Total Score = Accuracy Score (Max 90) + Bonus Score (Max 10)**

#### 1. Accuracy Score (0-90 points)
- Uses `difflib` pattern matching to compare your **Search Query** vs. the **Cleaned Title**.
- **Exact Matches** get closer to 90 points.
- **Partial Matches** (e.g., "Avengers" vs "Avengers Endgame") get lower scores proportional to the mismatch length.
- *Goal: Ensure the top result is exactly what you typed.*

#### 2. Tie-Breaker Bonus (Max 10 points)
Used to rank identical titles by quality, mirroring standard **Sonarr/Radarr Quality Profiles**.

**Profile Score (0-8 points):**
- **8**: `Remux` / `ISO` / `2160p` (with Source)
- **7**: `1080p BluRay`
- **6**: `1080p WEB-DL`
- **5**: `1080p` (Generic)
- **4**: `720p BluRay / WEB-DL`
- **3**: `720p` (Generic)
- **2**: `HDTV` / `PDTV`
- **1**: `SD` / `DVD` / `480p`
- **0**: `CAM` / `TS` / Low Quality

**Language Bonus (+2 points):**
- `Vietsub`, `Vietdub`, `Thuyết Minh`, `Lồng Tiếng` receive an additional **+2 points**.

### Example Calculation
**Query:** "Avengers"
1. **File A:** "Avengers.2012.2160p.Remux.Vietsub.mkv"
   - Accuracy: **90** (Exact match)
   - Profile: Remux (**8**) + Vietsub (**2**) = **10** (Max Bonus)
   - **Total: 100**

2. **File B:** "Avengers.2012.1080p.BluRay.mkv"
   - Accuracy: **90**
   - Profile: 1080p BluRay (**7**) + No Sub (**0**) = **7**
   - **Total: 97**

*Result: Remux > BluRay > WEB-DL.*

## 3. Supported Quality Types

For filtering purposes, the specific quality badges are detected as follows:

- **4K**: Contains `4k`, `2160p`, `uhd`
- **1080P**: Contains `1080p`
- **720P**: Contains `720p`
- **BluRay**: Contains `bluray`, `remux`, `bdrip`
- **WEB-DL**: Contains `web-dl`, `webdl`, `webrip`
- **HDTV**: Contains `hdtv`, `pdtv`
- **HDR**: Contains `hdr`, `dolby vision`, `dv`
- **SD**: Contains `480p`, `dvd`, `sd`
