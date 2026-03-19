use criterion::{black_box, criterion_group, criterion_main, Criterion};
use geo_polygonize_core::{Coord3D, Line3D, Polygonizer, PolygonizerOptions};
use std::f64::consts::PI;

fn generate_circle_points(
    center_x: f64,
    center_y: f64,
    radius: f64,
    num_points: usize,
) -> Vec<Coord3D> {
    let mut points = Vec::with_capacity(num_points + 1);
    for i in 0..num_points {
        let angle = 2.0 * PI * (i as f64) / (num_points as f64);
        points.push(Coord3D::new(
            center_x + radius * angle.cos(),
            center_y + radius * angle.sin(),
            0.0,
        ));
    }
    // Close the ring
    points.push(points[0]);
    points
}

fn generate_hole_sort_scenario(num_holes: usize, points_per_hole: usize) -> Vec<Line3D> {
    let mut lines = Vec::new();

    // Outer shell: a large circle CCW
    let shell_points = generate_circle_points(500.0, 500.0, 500.0, 100);
    for i in 0..shell_points.len() - 1 {
        lines.push(Line3D::new(shell_points[i], shell_points[i + 1], 0));
    }

    // Many holes: small circles CW
    for i in 0..num_holes {
        let x = (i % 30) as f64 * 30.0 + 50.0;
        let y = (i / 30) as f64 * 30.0 + 50.0;
        let mut hole_points = generate_circle_points(x, y, 10.0, points_per_hole);
        hole_points.reverse(); // Make it CW for hole
        for j in 0..hole_points.len() - 1 {
            lines.push(Line3D::new(
                hole_points[j],
                hole_points[j + 1],
                (i + 1) as u32,
            ));
        }
    }

    lines
}

fn bench_hole_sort(c: &mut Criterion) {
    let lines = generate_hole_sort_scenario(1000, 100);
    let options = PolygonizerOptions::default();

    c.bench_function("hole_sort_1000_holes_100_points", |b| {
        b.iter(|| {
            let mut poly = Polygonizer::with_options(options.clone());
            poly.add_lines(lines.clone());
            poly.polygonize().unwrap();
        });
    });
}

criterion_group!(benches, bench_hole_sort);
criterion_main!(benches);
