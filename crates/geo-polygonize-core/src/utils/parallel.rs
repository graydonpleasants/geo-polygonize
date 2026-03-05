#[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
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
where
    T: Sync,
    F: Fn(&T) + Sync + Send,
{
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
where
    T: Send,
    F: Fn(&mut T) + Sync + Send,
{
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

/// Helper for parallel flat_map
#[inline]
pub fn par_flat_map<T, F, U, R>(slice: &[T], f: F) -> Vec<R>
where
    T: Sync,
    F: Fn(&T) -> U + Sync + Send,
    U: IntoIterator<Item = R>,
    R: Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        slice.par_iter().flat_map_iter(f).collect()
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        slice.iter().flat_map(f).collect()
    }
}

/// Helper for parallel unstable sort
#[inline]
pub fn par_sort_unstable<T: Ord + Send>(slice: &mut [T]) {
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        slice.par_sort_unstable();
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        slice.sort_unstable();
    }
}

/// Helper for parallel zip and for_each
#[inline]
pub fn par_zip_for_each<A, B, F>(a: &mut [A], b: &[B], f: F)
where
    A: Send,
    B: Sync,
    F: Fn(&mut A, &B) + Sync + Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        a.par_iter_mut()
            .zip(b.par_iter())
            .for_each(|(x, y)| f(x, y));
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        a.iter_mut().zip(b.iter()).for_each(|(x, y)| f(x, y));
    }
}

/// Helper for parallel consume, enumerate, map and collect
#[inline]
pub fn par_into_enumerate_map<T, F, U>(vec: Vec<T>, f: F) -> Vec<U>
where
    T: Send,
    F: Fn((usize, T)) -> U + Sync + Send,
    U: Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        vec.into_par_iter().enumerate().map(f).collect()
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        vec.into_iter().enumerate().map(f).collect()
    }
}
