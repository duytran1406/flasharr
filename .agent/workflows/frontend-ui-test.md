---
description: Comprehensive UI/UX test for Flasharr v3 frontend
---

# Flasharr v3 Frontend UI Test Workflow

This workflow tests all UI components, buttons, and interactions in the Flasharr v3 frontend.

## Prerequisites

1. Backend server running at `http://localhost:8080`
2. Frontend dev server running at `http://localhost:5173`
3. Browser open to `http://localhost:5173`

---

## Test 1: Connection Status & Header

### 1.1 Check WebSocket Connection

- **Location:** Header (top right)
- **Expected:** Green dot (🟢) with "Connected" text
- **Test:** Observe connection indicator
- **Pass Criteria:** Shows "Connected" in green

### 1.2 Check Global Stats

- **Location:** Header (top right)
- **Expected:**
  - Speed pill showing current download speed (e.g., "5.2 MB/s")
  - Active downloads count (e.g., "3")
- **Test:** Verify numbers update in real-time
- **Pass Criteria:** Stats display and update automatically

### 1.3 Test Sidebar Navigation

- **Location:** Left sidebar
- **Test:** Click each navigation item:
  1. Dashboard
  2. Discover
  3. Explore
  4. Downloads
  5. Settings
- **Pass Criteria:** Each page loads without errors

---

## Test 2: Dashboard Page

### 2.1 Trending Section

- **Location:** Top carousel
- **Test:**
  - Click left/right arrows
  - Verify carousel scrolls
- **Pass Criteria:** Carousel navigation works

### 2.2 Stats Tiles

- **Location:** Right column, "Account Overview" section
- **Test:** Verify stats display:
  - ACTIVE (blue/secondary color)
  - QUEUED (yellow/warning color)
  - DONE (cyan/primary color)
- **Pass Criteria:** All three stats show correct numbers

### 2.3 Active Downloads Section

- **Location:** Left column, "Active Downloads"
- **Test:**
  - Verify top 5 active downloads display
  - Check progress bars animate
  - Click "EXPAND LIST" button
- **Pass Criteria:**
  - Downloads show with progress
  - Button navigates to /downloads

### 2.4 Account Card

- **Location:** Right column, top
- **Test:** Verify account info displays:
  - Email address
  - VIP badge
  - Quota information
- **Pass Criteria:** Account data loads from backend

---

## Test 3: Downloads Page - Filters

### 3.1 Filter Tabs

- **Location:** Top of downloads page
- **Test:** Click each filter tab:
  1. All
  2. Active
  3. Queued
  4. Completed
  5. Failed
  6. Paused
- **Pass Criteria:**
  - Each filter shows correct downloads
  - Count badges update
  - Active tab highlights in cyan

### 3.2 Search Functionality

- **Location:** Search box in filter bar
- **Test:**
  1. Type a filename
  2. Verify downloads filter in real-time
  3. Clear search
  4. Verify all downloads return
- **Pass Criteria:** Search filters downloads correctly

---

## Test 4: Downloads Page - Download Cards

### 4.1 Card Display

- **Location:** Downloads grid
- **Test:** Verify each card shows:
  - State icon (emoji)
  - Filename
  - Status badge (colored)
  - Progress (if downloading)
  - Size
  - Speed (if downloading)
  - ETA (if downloading)
  - Error message (if failed)
- **Pass Criteria:** All information displays correctly

### 4.2 Progress Bar Animation

- **Location:** Download cards (active downloads)
- **Test:**
  - Observe progress bar
  - Verify it updates in real-time
- **Pass Criteria:** Progress bar animates smoothly

---

## Test 5: Downloads Page - Action Buttons

### 5.1 Pause Button

- **Location:** Download card (active download)
- **Test:**
  1. Click pause button (⏸️)
  2. Verify download pauses
  3. Verify state changes to "PAUSED"
  4. Verify button changes to resume (▶️)
- **Pass Criteria:** Download pauses successfully

### 5.2 Resume Button

- **Location:** Download card (paused download)
- **Test:**
  1. Click resume button (▶️)
  2. Verify download resumes
  3. Verify state changes to "DOWNLOADING"
  4. Verify progress updates
- **Pass Criteria:** Download resumes successfully

### 5.3 Delete Button

- **Location:** Download card
- **Test:**
  1. Click delete button (🗑️)
  2. Verify confirmation dialog appears
  3. Click "OK"
  4. Verify download removed from list
- **Pass Criteria:** Download deletes successfully

### 5.4 Retry Button

- **Location:** Download card (failed download)
- **Test:**
  1. Click retry button (🔄)
  2. Verify download retries
  3. Verify state changes to "QUEUED" or "DOWNLOADING"
- **Pass Criteria:** Download retries successfully

---

## Test 6: Downloads Page - Batch Actions

### 6.1 Pause All

- **Location:** Top right, batch actions
- **Test:**
  1. Click "Pause All" button (red pause icon)
  2. Verify confirmation dialog
  3. Click "OK"
  4. Verify all active downloads pause
- **Pass Criteria:** All downloads pause

### 6.2 Resume All

- **Location:** Top right, batch actions
- **Test:**
  1. Click "Resume All" button (green play icon)
  2. Verify confirmation dialog
  3. Click "OK"
  4. Verify all paused downloads resume
- **Pass Criteria:** All downloads resume

---

## Test 7: Downloads Page - Context Menu

### 7.1 Open Context Menu

- **Location:** Any download card
- **Test:**
  1. Right-click on download card
  2. Verify context menu appears
- **Pass Criteria:** Menu shows at cursor position

### 7.2 Context Menu Actions

- **Location:** Context menu
- **Test:** Verify menu shows correct actions based on state:
  - **Downloading:** Pause Download
  - **Paused:** Resume Download
  - **Failed:** Retry Download
  - **All states:** Copy Fshare Link, Delete Task
- **Pass Criteria:** Correct actions display

### 7.3 Copy Fshare Link

- **Location:** Context menu
- **Test:**
  1. Click "Copy Fshare Link"
  2. Paste into notepad
  3. Verify correct URL copied
- **Pass Criteria:** URL copies to clipboard

### 7.4 Close Context Menu

- **Location:** Anywhere on page
- **Test:**
  1. Click outside context menu
  2. Verify menu closes
- **Pass Criteria:** Menu dismisses

---

## Test 8: Settings Page

### 8.1 Engine Configuration

- **Location:** Settings page, "Engine Configuration" card
- **Test:**
  1. Adjust "Max Concurrency" slider
  2. Adjust "Worker Threads" slider
  3. Verify values update
  4. Click "SAVE ENGINE CONFIG"
- **Pass Criteria:** Settings save successfully

### 8.2 Account Management

- **Location:** Settings page, "FSHARE ACCOUNT" card
- **Test:**
  1. Verify account displays
  2. Click refresh button (🔄)
  3. Verify account info refreshes
- **Pass Criteria:** Account info updates

### 8.3 Integration Settings

- **Location:** Settings page, right column
- **Test:**
  1. Enter Radarr URL
  2. Enter Radarr API key
  3. Click "TEST CONNECTION"
  4. Verify connection status updates
- **Pass Criteria:** Connection test works

---

## Test 9: Real-time Updates

### 9.1 WebSocket Updates

- **Test:**
  1. Add a download via backend/API
  2. Verify it appears in UI immediately
  3. Observe progress updates in real-time
- **Pass Criteria:** UI updates without refresh

### 9.2 Stats Updates

- **Test:**
  1. Pause a download
  2. Verify header stats update
  3. Verify dashboard stats update
- **Pass Criteria:** All stats update automatically

---

## Test 10: Responsive Design

### 10.1 Mobile View (375px)

- **Test:**
  1. Resize browser to 375px width
  2. Verify sidebar collapses
  3. Verify bottom navigation appears
  4. Verify downloads grid becomes single column
- **Pass Criteria:** Mobile layout works

### 10.2 Tablet View (768px)

- **Test:**
  1. Resize browser to 768px width
  2. Verify layout adjusts
  3. Verify all features accessible
- **Pass Criteria:** Tablet layout works

### 10.3 Desktop View (1920px)

- **Test:**
  1. Resize browser to 1920px width
  2. Verify full layout displays
  3. Verify all features accessible
- **Pass Criteria:** Desktop layout works

---

## Test 11: Error Handling

### 11.1 Network Error

- **Test:**
  1. Stop backend server
  2. Verify connection status shows "Disconnected"
  3. Verify error messages display
  4. Restart backend
  5. Verify reconnection works
- **Pass Criteria:** Graceful error handling

### 11.2 Failed Download

- **Test:**
  1. Add invalid URL
  2. Verify error message displays
  3. Verify retry button appears
- **Pass Criteria:** Error state handled correctly

---

## Test 12: Performance

### 12.1 Large Download List

- **Test:**
  1. Add 50+ downloads
  2. Verify page remains responsive
  3. Verify scrolling is smooth
  4. Verify filters work quickly
- **Pass Criteria:** No lag or freezing

### 12.2 Real-time Updates Performance

- **Test:**
  1. Have 10+ active downloads
  2. Verify progress updates smoothly
  3. Verify no UI stuttering
- **Pass Criteria:** Smooth animations

---

## Summary Checklist

- [ ] All navigation links work
- [ ] WebSocket connects successfully
- [ ] Dashboard displays real-time data
- [ ] All filter tabs work
- [ ] Search filters correctly
- [ ] Pause/resume/delete/retry work
- [ ] Batch actions work
- [ ] Context menu works
- [ ] Settings save correctly
- [ ] Real-time updates work
- [ ] Mobile responsive
- [ ] Error handling works
- [ ] Performance is good

---

## Bug Reporting Template

If you find a bug, report it with:

```
**Bug:** [Brief description]
**Location:** [Page and component]
**Steps to Reproduce:**
1. [Step 1]
2. [Step 2]
3. [Step 3]

**Expected:** [What should happen]
**Actual:** [What actually happened]
**Screenshot:** [If applicable]
**Console Errors:** [If any]
```

---

**Test completed:** [Date]  
**Tester:** [Name]  
**Result:** [Pass/Fail]  
**Notes:** [Any additional observations]
