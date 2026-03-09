class SimplePolygon:
    def __init__(self, shell, holes, shell_ids=None, holes_ids=None):
        self.shell = shell
        self.holes = holes
        self.shell_ids = shell_ids
        self.holes_ids = holes_ids

    @property
    def __geo_interface__(self):
        return {
            'type': 'Polygon',
            'coordinates': tuple([self.shell] + self.holes)
        }

    def __repr__(self):
        return f"<SimplePolygon shell_pts={len(self.shell)} holes={len(self.holes)}>"
