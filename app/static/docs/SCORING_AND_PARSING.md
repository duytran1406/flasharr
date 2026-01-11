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

## 2. Score System (Multi-Factor Matching)
Search results are scored from **0 to 100**, using an intelligent multi-factor matching algorithm.

### Formula
**Total Score = Accuracy Score (Max 90) + Bonus Score (Max 10)**

#### 1. Accuracy Score (0-90 points)
Uses **hierarchical matching** to determine relevance, from most to least specific:

**Matching Hierarchy:**

1. **Exact Match (90 points)**
   - Query exactly matches title (case-insensitive)
   - Example: "Avengers" → "Avengers"

2. **Word-Perfect Match (75-80 points)**
   - All query words found in title
   - **80 pts**: Words in same order → "Iron Man" → "Iron Man 2"
   - **75 pts**: Words present, different order → "Man Iron" → "Iron Man"

3. **Prefix Match (75 points)**
   - Title starts with query
   - Example: "Avengers" → "Avengers Endgame"

4. **Substring Match (70 points)**
   - Query is substring of title
   - Example: "endgame" → "Avengers Endgame"

5. **Token-Based Scoring (0-65 points)**
   - Partial word matches with relevance penalty
   - Formula: `(matched_words / query_words) × 65 - (extra_words × 2)`
   - Example: "Iron Man War" → "Iron Man" (43pts)

6. **Fuzzy Fallback (0-40 points)**
   - Character-based similarity for typos
   - Example: "Avengrs" → "Avengers" (~35pts)

**Key Features:**
- Word-based (not character-based) matching
- Exact matches always rank highest
- Extra words in title reduce relevance

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
