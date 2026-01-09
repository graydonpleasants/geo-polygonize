
#[cfg(feature = "parallel")]
use rayon::prelude::*;

/// A trait to switch between parallel and sequential iterators
pub trait MaybeParIter<T> {
    type Iter: Iterator<Item = T>;
    fn maybe_par_iter(self) -> Self::Iter;
}

// Helper to switch based on size or architecture
// Note: This helper runs for_each
#[inline]
pub fn iterate<T, F>(collection: &[T], f: F)
where T: Sync, F: Fn(&T) + Sync + Send {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        // Heuristic: Don't spin up Rayon for < 1000 items
        if collection.len() > 1000 {
            collection.par_iter().for_each(f);
        } else {
            collection.iter().for_each(f);
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection.iter().for_each(f);
    }
}

// Helper for mutable iteration
#[inline]
pub fn iterate_mut<T, F>(collection: &mut [T], f: F)
where T: Send, F: Fn(&mut T) + Sync + Send {
     #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        if collection.len() > 1000 {
            collection.par_iter_mut().for_each(f);
        } else {
            collection.iter_mut().for_each(f);
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection.iter_mut().for_each(f);
    }
}
