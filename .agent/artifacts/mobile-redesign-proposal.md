# Flasharr Mobile Redesign Proposal

## Executive Summary

The current mobile version of Flasharr has critical responsive design issues that make it nearly unusable on mobile devices. This proposal outlines a comprehensive redesign focusing on:

- **Card-based layouts** instead of tables
- **Vertical stacking** of content
- **Proper spacing** to eliminate overlapping elements
- **Touch-friendly** interactive elements
- **Consistent glassmorphism** design language

---

## Current Issues Identified

### 1. Dashboard Page

- ❌ Section headers overlap ("ACTIVE DOWNLOA" + "ACCOUNT OVERVIEW")
- ❌ Management buttons misaligned and clipping
- ❌ Trending carousel takes too much vertical space
- ❌ Desktop-first layout forced to shrink

### 2. Downloads Page

- ❌ **CRITICAL**: Table columns completely overlapping
- ❌ Headers mashed together ("STATUSSIZE", "PROGRESSSPEED")
- ❌ Impossible to read file statuses or progress
- ❌ Table layout fundamentally incompatible with mobile

### 3. Media Detail Page

- ❌ Movie poster overlaps title and tagline
- ❌ Tagline text squashed into narrow vertical column
- ❌ Metadata poorly spaced
- ❌ Similar titles grid doesn't scale properly

### 4. Settings Page

- ❌ Trending carousel incorrectly appears (component leak)
- ❌ Search bar cramped and hard to tap
- ❌ Fixed-width containers cause text truncation

---

## Redesign Solutions

### Dashboard Redesign

**Key Changes:**

- Maintain trending carousel but optimize size
- Convert download list to **card-based layout**
- Each download card shows:
  - Filename (large, readable)
  - Status badge (color-coded)
  - Progress bar (full width)
  - Size/Speed/Time remaining
  - Action buttons (pause/resume)
- Clear section separation
- Account overview moved below or to separate tab

### Downloads Page Redesign

**Key Changes:**

- **Replace table with vertical card list**
- Each download card contains:
  - Large filename at top
  - Status badge (green "COMPLETE", red "FAILED", cyan "DOWNLOADING")
  - File size on right
  - Full-width progress bar
  - Speed and timestamp below
- Control buttons (play all, pause all, refresh) in header
- Pagination at bottom
- Filter button easily accessible

### Media Detail Page Redesign

**Key Changes:**

- **Side-by-side poster + title layout**
  - Poster: 120px width, left-aligned
  - Title + tagline: right side with proper spacing
- Metadata row: year, rating, duration (horizontal)
- Genre tags as pills
- Full-width "SMART SEARCH" button
- Sections vertically stacked:
  - Status
  - Release dates
  - Tomatometer
  - Overview (scrollable text)
- Similar titles in scrollable horizontal carousel

---

## Design System Updates Needed

### Breakpoints

```css
/* Mobile First */
@media (max-width: 640px) {
  /* Stack everything vertically */
  /* Use card layouts */
  /* Increase touch target sizes */
}
```

### Mobile-Specific Components

#### Download Card Component

```html
<div class="download-card-mobile">
  <div class="card-header">
    <h3 class="filename">Movie_Avatar_2_4K_HDR.mkv</h3>
    <span class="status-badge downloading">DOWNLOADING</span>
  </div>
  <div class="card-stats">
    <span class="size">12.4 GB / 20.5 GB</span>
    <span class="speed">15 MB/s</span>
  </div>
  <div class="progress-bar">
    <div class="progress-fill" style="width: 60%"></div>
  </div>
  <div class="card-footer">
    <span class="time-remaining">2h 10m left</span>
    <button class="action-btn">⏸</button>
  </div>
</div>
```

#### Media Hero Component (Mobile)

```html
<div class="media-hero-mobile">
  <div class="hero-content">
    <img class="poster-small" src="..." alt="The Rip" />
    <div class="hero-text">
      <h1>The Rip</h1>
      <p class="tagline">Truth Unravels</p>
    </div>
  </div>
  <div class="metadata-row">
    <span class="year">📅 2026</span>
    <span class="rating">⭐ 7.1</span>
    <span class="duration">🕐 1h 53m</span>
  </div>
  <div class="genre-tags">
    <span class="tag">Sci-Fi</span>
    <span class="tag">Thriller</span>
    <span class="tag">Mystery</span>
  </div>
</div>
```

### CSS Variables for Mobile

```css
:root {
  /* Mobile spacing */
  --mobile-padding: 16px;
  --mobile-gap: 12px;
  --mobile-card-radius: 12px;

  /* Touch targets */
  --touch-target-min: 44px;

  /* Mobile typography */
  --mobile-h1: 24px;
  --mobile-h2: 20px;
  --mobile-h3: 16px;
  --mobile-body: 14px;
  --mobile-small: 12px;
}
```

---

## Implementation Priority

### Phase 1: Critical Fixes (High Priority)

1. ✅ Downloads page - Convert table to cards
2. ✅ Media detail page - Fix poster/title overlap
3. ✅ Add mobile-specific CSS breakpoints

### Phase 2: Layout Improvements (Medium Priority)

4. ✅ Dashboard - Card-based download list
5. ✅ Navigation - Ensure bottom nav is sticky
6. ✅ Settings - Fix component leak, improve layout

### Phase 3: Polish (Low Priority)

7. ✅ Add touch gestures (swipe to delete, pull to refresh)
8. ✅ Optimize carousel for mobile
9. ✅ Add haptic feedback for actions

---

## Technical Implementation Notes

### Files to Modify

1. **CSS Files**
   - `src/flasharr/static/css/styles.css` - Add mobile breakpoints
   - `src/flasharr/static/css/components.css` - Mobile card components
   - `src/flasharr/static/css/responsive.css` - New file for responsive utilities

2. **Template Files**
   - `src/flasharr/templates/downloads.html` - Add mobile card layout
   - `src/flasharr/templates/media_detail.html` - Restructure hero section
   - `src/flasharr/templates/dashboard.html` - Update download list

3. **JavaScript**
   - `src/flasharr/static/js/app.js` - Add viewport detection
   - Add touch event handlers for mobile interactions

### Testing Checklist

- [ ] iPhone SE (375x667)
- [ ] iPhone 12/13 (390x844)
- [ ] iPhone 14 Pro Max (430x932)
- [ ] Android (360x640)
- [ ] Tablet (768x1024)
- [ ] Landscape orientation
- [ ] Touch interactions
- [ ] Scroll performance

---

## Next Steps

1. **Review wireframes** with stakeholders
2. **Approve design direction**
3. **Begin Phase 1 implementation**
4. **Test on real devices**
5. **Iterate based on feedback**

---

## Wireframe References

See generated wireframes:

- `mobile_dashboard_redesign.png` - Dashboard with card layout
- `mobile_downloads_redesign.png` - Downloads page with vertical cards
- `mobile_media_redesign.png` - Media detail with fixed layout

---

_Generated: 2026-01-21_
_Status: Awaiting Approval_
