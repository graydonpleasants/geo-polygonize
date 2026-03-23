# Basic Square

A simple example showing the polygonization of a single unit square.

## Configuration

```json
{
  "node_input": false,
  "snap_grid_size": 0
}
```

## Input Geometry

```json
{
  "type": "FeatureCollection",
  "features": [
    {
      "type": "Feature",
      "properties": {},
      "geometry": {
        "type": "LineString",
        "coordinates": [
          [0.0, 0.0],
          [1.0, 0.0],
          [1.0, 1.0],
          [0.0, 1.0],
          [0.0, 0.0]
        ]
      }
    }
  ]
}

```

## Interactive Playground

You can experiment with this scenario in the [Playground](/playground/?scenario=basic-square).
