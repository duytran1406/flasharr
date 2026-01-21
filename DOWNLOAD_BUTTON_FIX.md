# Download Button Fix: Proper Error Handling

## Issue Report
**Date:** 2026-01-20  
**Endpoint:** `POST /api/downloads`  
**Problem:** Button shows success (✓) even when `success: false` in response

### Example Failure Case
```json
Request: {"url": "https://www.fshare.vn/file/K5PV16GK5R4Z?des=71f5561d15"}
Response: {"status": "ok", "success": false, "nzo_id": null}
```

**Old Behavior:**
- ✓ Button turns green (success)
- ✓ Button stays clickable
- ❌ No error message shown

**Expected Behavior:**
- ❌ Button shows error state
- 📢 Error toast notification appears
- 🔄 Button resets after 3 seconds

---

## Root Cause

**File:** `src/flasharr/static/js/app_v2.js` line 1611

```javascript
// OLD CODE - Only checked HTTP status
if (res.ok) {  // ← Only checks 200 OK, not actual success
    btn.style.background = '#10b981';
    btn.innerHTML = '<span class="material-icons">check</span>';
}
```

**Problem:** The API returns HTTP 200 even when the download fails to queue, with `success: false` in the JSON body. The old code only checked `res.ok` (HTTP status) and ignored the actual `success` field.

---

## The Fix

### 1. Check Both HTTP Status AND Success Field

```javascript
const data = await res.json();

// Check BOTH conditions
if (res.ok && data.status === 'ok' && data.success === true) {
    // TRUE success
    btn.style.background = '#10b981';
    btn.innerHTML = '<span class="material-icons">check</span>';
    btn.disabled = true;  // Keep disabled
    this.showToast('Download added to queue', 'success');
} else {
    // Failure
    btn.style.background = '#ef4444';
    btn.innerHTML = '<span class="material-icons">error</span>';
    this.showToast(data.message || 'Failed to add download', 'error');
    
    // Reset after 3 seconds
    setTimeout(() => {
        btn.style.background = 'var(--color-primary)';
        btn.innerHTML = originalHTML;
        btn.disabled = false;
    }, 3000);
}
```

### 2. Added Toast Notification System

**New Method:** `showToast(message, type)`

Features:
- ✅ Appears bottom-right corner
- ✅ Auto-dismisses after 3 seconds
- ✅ Smooth slide-in/out animations
- ✅ Color-coded by type (error=red, success=green, info=blue)
- ✅ Material icons for visual clarity
- ✅ Stacks multiple toasts vertically

**Toast Types:**
- `error` - Red background, error icon
- `success` - Green background, check icon
- `info` - Blue background, info icon

### 3. Button State Management

**Success State:**
```javascript
btn.disabled = true;  // Prevent re-clicking
btn.style.background = '#10b981';  // Green
btn.innerHTML = '<span class="material-icons">check</span>';
```

**Error State:**
```javascript
btn.disabled = true;  // Temporarily disabled
btn.style.background = '#ef4444';  // Red
btn.innerHTML = '<span class="material-icons">error</span>';

// Auto-reset after 3 seconds
setTimeout(() => {
    btn.disabled = false;  // Re-enable
    btn.style.background = 'var(--color-primary)';
    btn.innerHTML = originalHTML;
}, 3000);
```

---

## CSS Animations

**File:** `src/flasharr/static/css/style_v2.css`

Added slide animations for toasts:

```css
@keyframes slideInRight {
    from {
        transform: translateX(400px);
        opacity: 0;
    }
    to {
        transform: translateX(0);
        opacity: 1;
    }
}

@keyframes slideOutRight {
    from {
        transform: translateX(0);
        opacity: 1;
    }
    to {
        transform: translateX(400px);
        opacity: 0;
    }
}
```

---

## User Experience Flow

### Success Scenario
1. User clicks "Get" button
2. Button shows spinner (loading)
3. API returns `{success: true}`
4. ✅ Button turns green with checkmark
5. ✅ Button becomes unclickable
6. 📢 Green toast: "Download added to queue"
7. Toast auto-dismisses after 3s

### Failure Scenario
1. User clicks "Get" button
2. Button shows spinner (loading)
3. API returns `{success: false, message: "..."}`
4. ❌ Button turns red with error icon
5. 📢 Red toast shows error message
6. Toast auto-dismisses after 3s
7. 🔄 Button resets to original state
8. ✅ Button becomes clickable again

### Network Error Scenario
1. User clicks "Get" button
2. Button shows spinner (loading)
3. Network request fails (timeout, 500, etc.)
4. ❌ Button turns red with error icon
5. 📢 Red toast: "Network error: Failed to add download"
6. Toast auto-dismisses after 3s
7. 🔄 Button resets to original state
8. ✅ Button becomes clickable again

---

## API Response Validation

The fix now validates THREE conditions:

```javascript
res.ok           // HTTP 200-299
data.status === 'ok'    // API status field
data.success === true   // Actual success flag
```

**All three must be true** for success state.

---

## Toast Container

**Location:** Bottom-right corner  
**Z-index:** 10000 (above all content)  
**Stacking:** Vertical (newest on top)  
**Auto-cleanup:** Container removed when empty

**Styling:**
```javascript
position: fixed;
bottom: 2rem;
right: 2rem;
z-index: 10000;
display: flex;
flex-direction: column;
gap: 0.5rem;
```

---

## Testing

### Test Case 1: Successful Download
```bash
curl -X POST http://localhost:8484/api/downloads \
  -H "Content-Type: application/json" \
  -d '{"url": "https://www.fshare.vn/file/VALID_FILE"}'
```
**Expected:**
- Response: `{"status": "ok", "success": true, "nzo_id": "..."}`
- Button: Green checkmark, disabled
- Toast: Green "Download added to queue"

### Test Case 2: Failed Download
```bash
curl -X POST http://localhost:8484/api/downloads \
  -H "Content-Type: application/json" \
  -d '{"url": "https://www.fshare.vn/file/INVALID"}'
```
**Expected:**
- Response: `{"status": "ok", "success": false, "nzo_id": null}`
- Button: Red error icon, resets after 3s
- Toast: Red error message

### Test Case 3: Network Error
- Disconnect network
- Click "Get" button
**Expected:**
- Button: Red error icon, resets after 3s
- Toast: Red "Network error: Failed to add download"

---

## Files Modified

1. **`src/flasharr/static/js/app_v2.js`**
   - Fixed `downloadItem()` method (lines 1598-1626)
   - Added `showToast()` method (new)

2. **`src/flasharr/static/css/style_v2.css`**
   - Added toast animations (lines 2016-2036)

---

## Deployment

```bash
cd /etc/pve/fshare-arr-bridge
SKIP_GIT=true bash deploy.sh
```

**Status:** ✅ Deployed (2026-01-20 18:50)

---

## Benefits

1. **Clear Feedback:** Users immediately know if download succeeded or failed
2. **Error Messages:** Specific error messages help troubleshooting
3. **Prevent Duplicates:** Successful downloads disable button
4. **Retry Capability:** Failed downloads re-enable button after 3s
5. **Professional UX:** Smooth animations and clear visual states
6. **Reusable System:** Toast notifications can be used elsewhere in app

---

## Future Enhancements

- [ ] Add toast position configuration (top/bottom, left/right)
- [ ] Add toast duration configuration
- [ ] Add close button on toasts
- [ ] Add progress bar for long operations
- [ ] Add sound effects for success/error
- [ ] Add toast history/log
