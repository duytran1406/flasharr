# OceanLoad Sitemap & Functional Specification

## 1. System Overview
**OceanLoad** is a high-density download manager for Fshare.vn.
**Architecture:** Client-Server (SPA-like navigation via Sidebar).
**Theme:** Oceanholic (Dark) / Arctic (Light).

---

## 2. Sitemap Hierarchy

```text
(Root)
├── / (Dashboard) ................ Command Center (Stats & Health)
├── /downloads ................... File Manager (The Grid)
├── /search ...................... Discovery (Fshare Search)
└── /settings .................... Configuration (Account & System)
```

---

## 3. Detailed Functionality per Page

### A. Dashboard (`/`)
**Role:** Passive Monitoring ("The Heads-up Display").
*   **User Goal:** "Is my system healthy? How fast am I downloading right now?"
*   **Key Widgets:**
    1.  **NetFlow Graph:** Real-time area chart of Download/Upload speed over the last 60 seconds.
    2.  **Storage Ring:** Donut chart showing Disk Usage (Used vs Free) on the download volume.
    3.  **Account Status:** Premium validity badge (VIP/Free) and Daily Traffic quota meter.
*   **Actions:** None (Read-only).
*   **Data Requirements:** WebSocket feed (`stats.speed`, `stats.storage`, `stats.quota`, `stats.is_premium`, `stats.expiry_date`).

### B. Downloads (`/downloads`)
**Role:** Active Management ("The Workhorse").
*   **User Goal:** "Manage my queue. Pause bad downloads. Priority handling."
*   **Layout:**
    *   **Desktop:** High-density Data Table.
    *   **Mobile:** Card List.
*   **Key Actions:**
    *   **Add Download:** Open "Add Link" modal (For both File URL and Folder URL).
    *   **Control:** Details (open "Details Modal" Pause, Resume, Delete, Force Recheck (Context Menu).
    *   **Sort/Filter:** Sort by Name, Size, Progress, ETA, Added. Filter by Status (Active/Error).
*   **Data Requirements:** Full Download List (`downloads[]`), Socket events (`download.update`).

### C. Search (`/search`)
**Role:** Content Discovery.
*   **User Goal:** "Find file X on Fshare."
*   **functionality:**
    *   **Global Search:** Large input field.
    *   **Results:**
        *   **Direct Download:** "One-click Add" button on result cards.
        *   **Link Copy:** Copy URL to clipboard.
*   **Data Requirements:** `/api/search?q=...` endpoint.

### D. Settings (`/settings`)
**Role:** Configuration.
*   **User Goal:** "Login to Fshare", "Change download path".
*   **Sections:**
    1.  **Account:** Fshare Username/Password/Cookie credentials.
    2.  **Download Engine:** Max Concurrent Downloads, Speed Limit.
    3.  **Arr Suite:** API Key, API URL, API Port for Sonarr and Radarr.
    4.  **Paths:** Destination folder map.
*   **Actions:** Save Config, Test Connection.
