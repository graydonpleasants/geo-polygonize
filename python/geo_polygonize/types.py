class SimplePolygon:
    def __init__(self, shell, holes, shell_ids=None, holes_ids=None, provenance=None):
        self.shell = shell
        self.holes = holes
        self.shell_ids = shell_ids
        self.holes_ids = holes_ids
        self.provenance = provenance

    @property
    def __geo_interface__(self):
        def _to_tuple(coords):
            if hasattr(coords, "tolist"):
                return tuple(map(tuple, coords.tolist()))
            return coords

        shell_tuple = _to_tuple(self.shell)
        holes_tuples = [_to_tuple(h) for h in self.holes]

        return {
            'type': 'Polygon',
            'coordinates': tuple([shell_tuple] + holes_tuples)
        }

    def __repr__(self):
        return f"<SimplePolygon shell_pts={len(self.shell)} holes={len(self.holes)}>"
