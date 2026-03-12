from geo_polygonize import polygonize as polygonize_pyo3
import numpy as np

# Coordinates for square with tail and floating line
coords_data = [
    0.0, 0.0, 10.0, 0.0,
    10.0, 0.0, 10.0, 10.0,
    10.0, 10.0, 0.0, 10.0,
    0.0, 10.0, 0.0, 0.0,
    10.0, 10.0, 20.0, 20.0, # Tail
    30.0, 30.0, 40.0, 40.0  # Floating line
]
offsets_data = [0, 2, 4, 6, 8, 10, 12]

def check_result(result):
    assert isinstance(result, dict)
    assert 'polygons' in result
    assert 'dangles' in result

    polys = result['polygons']
    dangles = result['dangles']

    assert len(polys) == 1
    assert len(dangles) == 2

    dangle_segments = set()
    for d in dangles:
        # Normalize direction for comparison
        # d is list of (x,y) tuples
        p1 = d[0]
        p2 = d[1]
        if p1 > p2:
            p1, p2 = p2, p1
        dangle_segments.add((p1, p2))

    expected_tail = ((10.0, 10.0), (20.0, 20.0))
    expected_floating = ((30.0, 30.0), (40.0, 40.0))

    assert expected_tail in dangle_segments
    assert expected_floating in dangle_segments

def test_square_with_tail_pyo3():
    print("Testing PyO3 implementation")
    result = polygonize_pyo3(np.array(coords_data), np.array(offsets_data, dtype=np.uint32))
    check_result(result)

