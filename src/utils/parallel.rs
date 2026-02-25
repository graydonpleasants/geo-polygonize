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

/// Helper for sorting a slice, using unstable sort.
#[inline]
pub fn sort_unstable<T>(collection: &mut [T])
where
    T: Ord + Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        if collection.len() > 1000 {
            collection.par_sort_unstable();
        } else {
            collection.sort_unstable();
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection.sort_unstable();
    }
}

/// Helper for flat_map + collect.
#[inline]
pub fn flat_map<T, U, I, F>(collection: &[T], f: F) -> Vec<U>
where
    T: Sync,
    U: Send,
    I: IntoIterator<Item = U>,
    F: Fn(&T) -> I + Sync + Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        if collection.len() > 1000 {
            collection.par_iter().flat_map_iter(f).collect()
        } else {
            collection.iter().flat_map(f).collect()
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection.iter().flat_map(f).collect()
    }
}

/// Helper for zip + for_each on two slices (one mutable, one immutable).
#[inline]
pub fn zip_for_each<T, U, F>(collection1: &mut [T], collection2: &[U], f: F)
where
    T: Send,
    U: Sync,
    F: Fn((&mut T, &U)) + Sync + Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        if collection1.len() > 1000 {
            collection1
                .par_iter_mut()
                .zip(collection2.par_iter())
                .for_each(f);
        } else {
            collection1.iter_mut().zip(collection2.iter()).for_each(f);
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection1.iter_mut().zip(collection2.iter()).for_each(f);
    }
}

/// Helper for into_iter + enumerate + map + collect on a Vec.
#[inline]
pub fn into_map_enumerate<T, U, F>(collection: Vec<T>, f: F) -> Vec<U>
where
    T: Send,
    U: Send,
    F: Fn((usize, T)) -> U + Sync + Send,
{
    #[cfg(all(feature = "parallel", not(target_arch = "wasm32")))]
    {
        if collection.len() > 1000 {
            collection.into_par_iter().enumerate().map(f).collect()
        } else {
            collection.into_iter().enumerate().map(f).collect()
        }
    }
    #[cfg(any(not(feature = "parallel"), target_arch = "wasm32"))]
    {
        collection.into_iter().enumerate().map(f).collect()
    }
}
