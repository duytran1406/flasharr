# UI Template Knowledge Map

**Source A:** [MUI Dashboard Template](https://github.com/mui/material-ui/tree/v7.3.7/docs/data/material/getting-started/templates/dashboard)
**Source B:** [MUI CRUD Dashboard](https://github.com/mui/material-ui/tree/v7.3.7/docs/data/material/getting-started/templates/crud-dashboard)

## 1. App Shell (From Source A)
We use the *Dashboard* template for the main application structure.
* **Navigation:** Fixed left `Drawer` (collapsible on mobile) containing the list of filters (All, Downloading, Completed).
* **Header:** `AppBar` containing the "Global Speed" stats and Theme Toggle.
* **Main Content:** The central `<Box>` where pages are rendered.

## 2. The Download List (From Source B)
We use the *CRUD* template for the main "Downloads" view.
* **Data Table:** Uses `<DataGrid />` (MUI X).
    * *Adaptation:* The "Status" column must be rendered with our custom Chip colors (Teal/Red).
    * *Adaptation:* The "Progress" column must use `<LinearProgress />`.
* **Toolbar:** The "Search" and "Filter" row above the table.
* **Action:** The "Add" button (top right in template) is replaced by our global FAB (Floating Action Button).

## 3. Visual Retrofit (M3 -> M2)
**Warning:** These templates use Material Design 3 (Rounded corners, surface colors).
**Strict M2 Override:**
* **Cards:** Force `borderRadius: 4px` (not 16px).
* **Elevation:** Use Shadows (`boxShadow`), not Surface Colors (`bgcolor`).
* **Buttons:** Uppercase text, medium ripple, `borderRadius: 4px`.