use crate::noding::snap::SnapNoder;
use crate::options::ZPolicy;
use crate::types::Line3D;

/// Compatibility wrapper for the retired experimental sweep-line noder.
///
/// The former implementation did not maintain sweep status across crossings and
/// could miss intersections. Exact `SnapNoder` noding is the single correctness
/// path until a distinct advanced backend demonstrates a measurable advantage.
/// This alias uses `grid_size = 0.0`, so no distance tolerance is applied.
pub struct AdvancedNoder {
    z_policy: ZPolicy,
}

impl Default for AdvancedNoder {
    fn default() -> Self {
        Self::new()
    }
}

impl AdvancedNoder {
    pub fn new() -> Self {
        Self {
            z_policy: ZPolicy::InterpolateAlongEdge,
        }
    }

    pub fn with_z_policy(mut self, z_policy: ZPolicy) -> Self {
        self.z_policy = z_policy;
        self
    }

    pub fn node(&self, lines: Vec<Line3D>) -> Vec<Line3D> {
        SnapNoder::new(0.0).with_z_policy(self.z_policy).node(lines)
    }
}
