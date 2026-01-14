# Handling Data Tables on Mobile

You cannot simply shrink a 10-column table to a mobile screen. You must transform it.

## Transformation Pattern: "Row to Card"
On screens < 600px, the `DataGrid` row transforms into a detailed `Card`.

### The Card Layout (Mobile)
* **Top Row:** Filename (Truncated middle if needed: `MyLong...File.mkv`).
* **Middle Row:** A large, thick Progress Bar.
* **Bottom Left:** Status Icon + Text (e.g., ⬇️ 5.2 MB/s).
* **Bottom Right:** ETA (e.g., 10m 30s).

### The List Layout (Desktop)
* Standard Table.
* **Sortable Headers:** Clicking "Size" sorts by size.
* **Resizable Columns:** Users should be able to drag column widths.