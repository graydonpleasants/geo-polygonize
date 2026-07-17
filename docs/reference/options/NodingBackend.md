# NodingBackend

## Variants

### `Snap`

Snap-rounding noder using the configured precision grid.

### `Advanced`

Deprecated compatibility alias for exact (`grid_size = 0`) snap noding. The experimental
sweep-line implementation was retired because it could miss intersections after the active
segment order changed at crossings.
