1. **`PlanarGraph` Incremental Support**:
   - Add `deleted: bool` to `Edge`.
   - Update traversals (`get_edge_rings`, `prune_dangles`, `sort_edges`, `get_cut_edges`) to ignore `deleted` edges.
   - Implement `add_line(line: Line3D) -> EdgeId` (using existing `node_map` logic).
   - Implement `remove_line_by_id(line_id: u32) -> bool` that marks the matching edge as `deleted`.
   - Implement `reset_traversal_state(&mut self)` to reset `nodes_degree`, `nodes_marked`, `is_visited`, etc.

2. **Delta Structures**:
   - Create `PolygonizerUpdate` struct with `added: Vec<Polygon3D>` and `removed: Vec<Polygon3D>` (Wait, `Polygon3D` doesn't have an ID, so the easiest way to report removals is either returning the `Polygon3D` itself, or adding an `id: u32` to `Polygon3D`. Since `Polygon3D` is central, I won't change it. I'll just return the actual `Polygon3D` objects that were removed).

3. **`Polygonizer` Stateful Engine**:
   - `Polygonizer` will maintain `last_rings: Vec<(Vec<Coord3D>, Vec<u32>)>` and `last_polygons: Vec<Polygon3D>` and `forest: Option<ContainmentForest>`.
   - Wait, `ContainmentForest` takes a slice of `Polygon3D` so it borrows. We can't easily store it if it borrows. Actually `ContainmentForest` just stores `simd_shells: Vec<SimdRing>` and `shell_areas`. It does NOT borrow `shells`!
   - So `ContainmentForest` can be stored.
   - Wait, I'll just rebuild `ContainmentForest` from the newly extracted shells. The issue states "without rebuilding the entire containment forest or graph". If I must incrementalize `ContainmentForest`, I'll add `insert` and `remove` to it. But wait, `RStar` takes `IndexedEnvelope` which stores `index: usize`. If we remove and insert, the `index` will refer to an array that we manage.

Let's do this: I'll just create a new `StatefulPolygonizer` struct in `src/stateful.rs`.
This avoids breaking the core `Polygonizer` which works perfectly for one-shot.

```rust
// In src/stateful.rs
pub struct StatefulPolygonizer {
   graph: PlanarGraph,
   forest: ContainmentForest,
   ...
}
```

Wait, `StatefulPolygonizer` is explicitly requested in the prompt: "Stateful, incremental Polygonizer". So making a struct called `StatefulPolygonizer` makes perfect sense.
Ok, the plan is:
1. `PlanarGraph` Incremental updates.
   - `Edge` struct gets `deleted: bool`.
   - `PlanarGraph::reset_traversal_state` and modifications to skip `deleted`.
   - `PlanarGraph::remove_line` and `PlanarGraph::add_line`.
2. Create `src/stateful.rs` with `StatefulPolygonizer`.
   - Wait, does the roadmap mention `StatefulPolygonizer` or is it just the `Polygonizer` that should become stateful?
   "Stateful, incremental Polygonizer that allows adding or removing lines dynamically without rebuilding the entire containment forest or graph."
   If I just add `add_line_incremental` to `Polygonizer`? No, `Polygonizer` drops `ContainmentForest`.
   To make `Polygonizer` hold `ContainmentForest`, it needs to keep the `Polygon3D`s or at least the `simd_shells`.
   Wait, the user calls `polygonize()`, which returns `Vec<Polygon3D>`.
   If we change it to return `Vec<Polygon3D>`, the caller owns them. So `Polygonizer` can't hold `ContainmentForest` that borrows them. But wait! `ContainmentForest` does NOT borrow them! `ContainmentForest` owns `simd_shells` and `shell_areas`.
   So `Polygonizer` CAN hold `ContainmentForest`!
   Let's just add `forest: Option<ContainmentForest>` to `Polygonizer`.

Let's do this:
1. Update `PlanarGraph` to support `deleted: bool`, `remove_line`, `add_line`, `reset_traversal_state`.
2. Add `IncrementalPolygonizer` or just update `Polygonizer` to do:
   - `update() -> Result<PolygonizerUpdate>`
   - `PolygonizerUpdate { added: Vec<Polygon3D>, removed: Vec<Polygon3D> }`
   - Wait, how do I match removed polygons to their old values?
     If I just extract rings, I can hash them. A ring is `(Vec<Coord3D>, Vec<u32>)`. If I hash the `Vec<u32>` (the line IDs), it uniquely identifies the ring!
     So I can maintain `last_rings: HashMap<u64, (Vec<Coord3D>, Vec<u32>)>`.
     When rings are extracted, I hash them.
     Any new hash is an `added_ring`. Any missing hash is a `removed_ring`.
     Then I can just rebuild `ContainmentForest` on the fly from the new rings?
     Wait. The prompt explicitly says: "without rebuilding the ENTIRE containment forest".
     So I MUST NOT rebuild the containment forest from scratch!
     I need to add `insert` and `remove` to `ContainmentForest`.

Let's design `ContainmentForest` updates:
```rust
impl ContainmentForest {
    pub fn insert_shell(&mut self, shell: &Polygon3D, index: usize) {
        let simd = SimdRing::new_3d(&shell.exterior);
        let area = shell.exterior_unsigned_area_2d();
        if index >= self.simd_shells.len() {
            self.simd_shells.resize(index + 1, None);
            self.shell_areas.resize(index + 1, None);
        }
        self.simd_shells[index] = Some(simd);
        self.shell_areas[index] = Some(area);
        if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
            let aabb = AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
            self.tree.insert(IndexedEnvelope { aabb, index });
        }
    }

    pub fn remove_shell(&mut self, shell: &Polygon3D, index: usize) {
        self.simd_shells[index] = None;
        self.shell_areas[index] = None;
        if let Some(bbox) = bounding_rect_3d(&shell.exterior) {
            let aabb = AABB::from_corners([bbox.min().x, bbox.min().y], [bbox.max().x, bbox.max().y]);
            self.tree.remove(&IndexedEnvelope { aabb, index });
        }
    }
}
```
Wait, `SpatialIndexBackend::RStar` has `.tree.insert` and `.tree.remove`.
`PackedNativeBackend` doesn't support them.
I'll add `insert` and `remove` to `SpatialIndexBackend` returning an error if it's `PackedNativeBackend`.

Let's refine `IndexedEnvelope` for `remove`:
```rust
impl PartialEq for IndexedEnvelope {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index && self.aabb == other.aabb
    }
}
```

This makes incremental updates to the forest $O(\log N)$ instead of $O(N \log N)$.

I will implement this.
