#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct CandidatePair {
    pub(crate) first: usize,
    pub(crate) second: usize,
}

pub mod grid;
pub mod hot_pixel;
pub mod snap;
pub mod sweep;
pub mod validate;
