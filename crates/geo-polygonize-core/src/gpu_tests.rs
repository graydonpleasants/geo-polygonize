#[cfg(test)]
mod tests {
    use crate::gpu::{GpuContainmentContext, GpuCoord, GpuRing, GpuPoint};

    #[test]
    fn test_gpu_point_in_polygon() {
        let ctx = GpuContainmentContext::new();
        // Since WebGPU initialization might fail in CI headless env, we handle it gracefully
        if ctx.is_none() {
            println!("Skipping GPU test because WebGPU adapter is not available in this environment.");
            return;
        }
        let gpu = ctx.unwrap();

        // A simple square ring: (0,0) -> (10,0) -> (10,10) -> (0,10) -> (0,0)
        let coords = vec![
            GpuCoord { x: 0.0, y: 0.0 },
            GpuCoord { x: 10.0, y: 0.0 },
            GpuCoord { x: 10.0, y: 10.0 },
            GpuCoord { x: 0.0, y: 10.0 },
            GpuCoord { x: 0.0, y: 0.0 },
        ];

        // Ring metadata
        let rings = vec![
            GpuRing { start_idx: 0, length: 5 },
            GpuRing { start_idx: 0, length: 5 },
        ];

        // Probe points: one inside (5,5), one outside (15,15)
        let points = vec![
            GpuPoint { x: 5.0, y: 5.0 },
            GpuPoint { x: 15.0, y: 15.0 },
        ];

        let results = gpu.check_containment(&coords, &rings, &points);
        assert_eq!(results.len(), 2);
        assert!(results[0], "Point (5,5) should be inside the square");
        assert!(!results[1], "Point (15,15) should be outside the square");
    }
}
