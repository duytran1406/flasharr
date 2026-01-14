# UX Patterns for Download Managers

## 1. The "Add Download" Flow
This is the most frequent user action. It needs to be frictionless.

1.  **Trigger:** FAB (Mobile) or Top Bar Button (Desktop).
2.  **Input:** A Dialog appears.
    * *Smart Paste:* If the clipboard contains a Magnet link or URL, auto-fill it.
    * *File Picker:* Drag & Drop zone for .torrent files.
3.  **Metadata Fetch (The "Loading" State):** Before adding, the app often needs to fetch metadata (file list). Show a spinner *inside* the dialog while this happens.
4.  **Confirmation:** User selects specific files from the torrent and Destination Path -> Clicks "Start".

## 2. Real-Time Data Updates
Data like "Speed" (2.4 MB/s) changes every second.
* **Monospaced Fonts:** Use monospaced fonts (e.g., Roboto Mono) for numbers (Speed, ETA, Size) to prevent the layout from "jittering" as character widths change.
* **Visual Stability:** Reserve space for the maximum character count (e.g., "999.9 MB/s") so columns don't resize constantly.

## 3. Bulk Actions
Users often need to "Pause All" or "Delete Selected".
* **Desktop:** Ctrl/Shift + Click selection -> Toolbar updates to show context actions.
* **Mobile:** Long-press to enter "Selection Mode".