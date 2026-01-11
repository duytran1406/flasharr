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

## 2. Quality Scoring System

Each search result is assigned a score from **0 to 100** to help users identify the highest quality releases.

**Base Score:** `50 points`

### Bonus Points

| Category | Condition | Points |
| :--- | :--- | :--- |
| **Resolution** | `4K`, `2160p`, `UHD` | **+50** |
| | `1080p` | **+30** |
| | `720p` | **+15** |
| **Visual Tech** | `HDR`, `HDR10`, `Dolby Vision`, `DV` | **+10** |
| **Audio** | `Atmos`, `TrueHD`, `DTS` | **+5** |
| **Codec** | `x265`, `HEVC`, `H.265` | **+5** |
| **Localization** | `Vietsub`, `Vietdub`, `Thuyết Minh`, `Lồng Tiếng` | **+10** |

*Note: The total score is capped at 100.*

### Example Calculation

**Filename:** `Avengers.Endgame.2019.2160p.HDR.Atmos.Vietsub.mkv`

- **Base**: 50
- **Resolution (2160p)**: +50
- **HDR**: +10
- **Audio (Atmos)**: +5
- **Localization (Vietsub)**: +10
- **Total**: 125 -> **Capped at 100**

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
