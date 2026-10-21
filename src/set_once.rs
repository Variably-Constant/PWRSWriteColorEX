//! A value built the first time it is read, then read from any thread without waiting.

use std::marker::PhantomData;
use std::ptr;
use std::sync::atomic::{AtomicPtr, Ordering};

/// A value built the first time it is read. Threads that read it first at the same moment each
/// build a value; the value stored first is the one every reader gets, the others are dropped,
/// and no thread waits on another. So a build may run more than once and must be harmless to
/// repeat.
pub struct SetOnce<T> {
    value: AtomicPtr<T>,
    /// Neither Send nor Sync; the impls below give the cell those of a value shared between
    /// threads.
    owns: PhantomData<*mut T>,
}

impl<T> SetOnce<T> {
    /// A cell with no value yet.
    pub const fn new() -> Self {
        Self { value: AtomicPtr::new(ptr::null_mut()), owns: PhantomData }
    }

    /// The value, built with `build` when there is none yet.
    pub fn get_or_init(&self, build: impl FnOnce() -> T) -> &T {
        let stored = self.value.load(Ordering::Acquire);
        if !stored.is_null() {
            // SAFETY: a stored pointer came from Box::into_raw below and is freed only when the
            // cell is dropped, which this shared borrow of the cell rules out.
            return unsafe { &*stored };
        }
        let built = Box::into_raw(Box::new(build()));
        match self.value.compare_exchange(ptr::null_mut(), built, Ordering::AcqRel, Ordering::Acquire) {
            // SAFETY: as above; `built` is now the stored value.
            Ok(_) => unsafe { &*built },
            Err(stored) => {
                // SAFETY: `built` lost the exchange, so no other thread ever saw it.
                drop(unsafe { Box::from_raw(built) });
                // SAFETY: as for the first load.
                unsafe { &*stored }
            }
        }
    }
}

impl<T> Default for SetOnce<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Drop for SetOnce<T> {
    fn drop(&mut self) {
        let stored = *self.value.get_mut();
        if !stored.is_null() {
            // SAFETY: the cell owns its stored value, and `&mut self` means no borrow of it is left.
            drop(unsafe { Box::from_raw(stored) });
        }
    }
}

// SAFETY: a value built on one thread is read from others and dropped on whichever thread drops
// the cell, as a value shared through an Arc is, so the cell is Sync when the value is Send and
// Sync.
unsafe impl<T: Send + Sync> Sync for SetOnce<T> {}
// SAFETY: moving the cell moves the value it owns.
unsafe impl<T: Send> Send for SetOnce<T> {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, AtomicUsize};

    #[test]
    fn the_first_value_stored_is_the_one_every_reader_gets() {
        let cell = SetOnce::new();
        assert_eq!(*cell.get_or_init(|| 7), 7);
        assert_eq!(*cell.get_or_init(|| 8), 7);
    }

    /// A value that counts how many of its kind are dropped.
    struct Counted<'a>(&'a AtomicUsize);

    impl Drop for Counted<'_> {
        fn drop(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }

    #[test]
    fn readers_racing_to_build_keep_one_value_and_drop_the_others() {
        const THREADS: usize = 8;
        let built = AtomicUsize::new(0);
        let dropped = AtomicUsize::new(0);
        let go = AtomicBool::new(false);
        {
            let cell = SetOnce::new();
            let seen: Vec<usize> = std::thread::scope(|scope| {
                let readers: Vec<_> = (0..THREADS)
                    .map(|_| {
                        let (cell, built, dropped, go) = (&cell, &built, &dropped, &go);
                        scope.spawn(move || {
                            while !go.load(Ordering::Acquire) {
                                std::hint::spin_loop();
                            }
                            let value = cell.get_or_init(|| {
                                built.fetch_add(1, Ordering::SeqCst);
                                Counted(dropped)
                            });
                            ptr::from_ref(value) as usize
                        })
                    })
                    .collect();
                go.store(true, Ordering::Release);
                readers.into_iter().map(|reader| reader.join().unwrap()).collect()
            });
            assert!(seen.iter().all(|&address| address == seen[0]), "every reader gets the same value");
            assert_eq!(dropped.load(Ordering::SeqCst), built.load(Ordering::SeqCst) - 1, "every value but the stored one is dropped");
        }
        assert_eq!(dropped.load(Ordering::SeqCst), built.load(Ordering::SeqCst), "the stored value is dropped with the cell");
    }
}
