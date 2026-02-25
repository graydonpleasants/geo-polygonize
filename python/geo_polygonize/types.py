class SimplePolygon:
    def __init__(self, shell, holes):
        self.shell = shell
        self.holes = holes

    @property
    def __geo_interface__(self):
        return {
            'type': 'Polygon',
            'coordinates': tuple([self.shell] + self.holes)
        }

    def __repr__(self):
        return f"<SimplePolygon shell_pts={len(self.shell)} holes={len(self.holes)}>"
