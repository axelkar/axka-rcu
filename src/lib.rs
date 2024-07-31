//! A reference-counted read-copy-update (RCU) primitive useful for protecting shared data
//!
//! ## Example
//!
//! ```
//! # use std::{thread::sleep, time::Duration};
#![cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
#![cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
//! use axka_rcu::Rcu;
//!
//! #[derive(Clone, Debug, PartialEq)]
//! struct Player {
//!     name: &'static str,
//!     points: usize
//! }
//!
//! let players = Arc::new(Rcu::new(Arc::new(vec![
//!     Player { name: "foo", points: 100 }
//! ])));
//! let players2 = players.clone();
//!
//! // Lock-free writing
//! std::thread::spawn(move || players2.update(|players| {
//!     sleep(Duration::from_millis(50));
//!     players.push(Player {
//!         name: "bar",
//!         points: players[0].points + 50
//!     })
//! }));
//!
//! // Lock-free reading
//! assert_eq!(*players.read(), [
//!     Player { name: "foo", points: 100 }
//! ]);
//!
//! sleep(Duration::from_millis(60));
//! assert_eq!(*players.read(), [
//!     Player { name: "foo", points: 100 },
//!     Player { name: "bar", points: 150 }
//! ]);
//! ```
//!
//! See the [`Rcu`] documentation for more details.
//!
//! ## Feature flags
#![doc = document_features::document_features!()]
#![cfg_attr(all(feature = "triomphe", not(test)), no_std)]

use core::{
    fmt,
    sync::atomic::{AtomicPtr, Ordering},
};

// Pick the correct Arc
#[cfg(not(feature = "triomphe"))]
use std::sync::Arc;
#[cfg(feature = "triomphe")]
use triomphe::Arc;

// Re-export the library
#[cfg(feature = "triomphe")]
pub use triomphe;

#[cfg(doctest)]
#[cfg(not(feature = "triomphe"))]
#[doc = include_str!("../README.md")]
extern "C" {}

// TODO: lists & reference block as in the video https://www.youtube.com/watch?v=rxQ5K9lo034

impl<T> Drop for Rcu<T> {
    fn drop(&mut self) {
        let ptr = self.ptr.load(Ordering::Acquire);

        // Decrement the reference count of the inner Arc<T> when all references to the Rcu are lost
        unsafe {
            // SAFETY: The ptr was created by Arc::into_raw in either Rcu::new or Rcu::write
            drop(Arc::from_raw(ptr));
        }
    }
}

/// A reference-counted read-copy-update (RCU) primitive useful for protecting shared data
///
/// It has a concept of "versions". The current version may be changed at any point and is managed
/// by an [`Arc`].
///
/// When a new version is created by [`write`](Self::write) and [`update`](Self::update), the old
/// version is dropped unless an [`Arc`] returned by [`read`](Self::read) exists.
///
/// # Example
///
/// ```
/// # use std::{thread::sleep, time::Duration};
#[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
#[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
/// #[derive(Clone, Debug, PartialEq)]
/// struct Player {
///     name: &'static str,
///     points: usize
/// }
///
/// use axka_rcu::Rcu;
/// let players = Arc::new(Rcu::new(Arc::new(vec![
///     Player { name: "foo", points: 100 }
/// ])));
/// let players2 = players.clone();
///
/// // Lock-free writing*
/// std::thread::spawn(move || players2.update(|players| {
///     sleep(Duration::from_millis(50));
///     players.push(Player {
///         name: "bar",
///         points: players[0].points + 50
///     })
/// }));
///
/// // Lock-free reading
/// assert_eq!(*players.read(), [
///     Player { name: "foo", points: 100 }
/// ]);
///
/// sleep(Duration::from_millis(60));
/// assert_eq!(*players.read(), [
///     Player { name: "foo", points: 100 },
///     Player { name: "bar", points: 150 }
/// ]);
/// ```
///
/// \*With a possibility of unintended overwriting, see [`update`](Self::update)
pub struct Rcu<T> {
    /// The "inner [`Arc`]" or the current version Arc
    ///
    /// Around the `T` of `AtomicPtr<T>`, is `ArcInner`. It is what defines a "version".
    /// Its strong count is the number of `Arc`s lent out by [`Rcu::read`], plus one if it's the
    /// current version.
    ptr: AtomicPtr<T>,
}

impl<T> Rcu<T> {
    /// Creates a new `Rcu` containing the given value.
    ///
    /// # Example
    ///
    /// ```
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu1 = Arc::new(Rcu::new(Arc::new("foo")));
    /// let rcu2 = rcu1.clone();
    ///
    /// rcu1.write(Arc::new("bar"));
    /// assert_eq!(*rcu2.read(), "bar");
    /// ```
    pub fn new(value: Arc<T>) -> Self {
        let ptr = Arc::into_raw(value) as *mut _;

        Self {
            ptr: AtomicPtr::new(ptr),
        }
    }

    /// Clones the [`Arc`] of the current version.
    ///
    /// # Example
    ///
    /// ```
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu = Rcu::new(Arc::new("foo bar"));
    /// assert_eq!(*rcu.read(), "foo bar");
    /// ```
    pub fn read(&self) -> Arc<T> {
        let ptr = self.ptr.load(Ordering::Acquire);
        #[cfg(not(feature = "triomphe"))]
        unsafe {
            // Increment the reference count of the inner Arc<T>
            // SAFETY:
            // - The ptr was created by Arc::into_raw in either Rcu::new or Rcu::write
            // - RcuInner counts as one strong reference
            Arc::increment_strong_count(ptr);
            // SAFETY: The ptr was created by Arc::into_raw in either Rcu::new or Rcu::write
            Arc::from_raw(ptr)
        }
        #[cfg(feature = "triomphe")]
        unsafe {
            let arc = Arc::from_raw(ptr);
            let _ = core::mem::ManuallyDrop::new(Arc::clone(&arc));
            arc
        }
    }

    /// Returns a reference to the current version.
    ///
    /// # Safety
    ///
    /// - This function and the returned reference are only safe when there is no writer.
    /// - If the RCU gets written to at any time, the returned reference is undefined behaviour.
    ///
    /// # Example
    ///
    /// ```
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu = Rcu::new(Arc::new("foo bar"));
    /// assert_eq!(unsafe { rcu.read_ref() }, &"foo bar");
    /// ```
    /// # UB Example
    /// ```no_run
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu = Rcu::new(Arc::new([42u8; 1024]));
    ///
    /// let r = unsafe { rcu.read_ref() };
    /// assert_eq!(r[0], 42);
    ///
    /// rcu.write(Arc::new([1; 1024]));
    ///
    /// assert_ne!(r[0], 42);
    /// ```
    pub unsafe fn read_ref(&self) -> &T {
        unsafe { &**self.ptr.as_ptr() }
    }

    /// Clones `T`, runs `updater` on `T` and [`write`](Self::write)s `T`.
    ///
    /// If you want to guarantee no **data loss** or unintended overwriting, use a semaphore on
    /// writes.
    ///
    /// # Example
    ///
    /// ```
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu = Rcu::new(Arc::new("foo".to_owned()));
    ///
    /// rcu.update(|s| s.push_str(" bar"));
    /// assert_eq!(*rcu.read(), "foo bar");
    /// ```
    pub fn update<F, R>(&self, updater: F)
    where
        T: Clone,
        F: FnOnce(&mut T) -> R,
    {
        // TODO: If there *is* a semaphore on Rcu::update and Rcu::write, it's guaranteed that the
        // internal pointer will not be updated during `updater` and it can be cloned without
        // atomic operations:
        // unsafe { &**self.ptr.as_ptr() }.clone()

        let mut value = (*self.read()).clone();
        updater(&mut value);
        self.write(Arc::new(value))
    }

    /// Writes a new version.
    ///
    /// # Example
    ///
    /// ```
    #[cfg_attr(feature = "triomphe", doc = "# use triomphe::Arc;")]
    #[cfg_attr(not(feature = "triomphe"), doc = "# use std::sync::Arc;")]
    /// use axka_rcu::Rcu;
    /// let rcu = Rcu::new(Arc::new("foo"));
    ///
    /// rcu.write(Arc::new("bar"));
    /// assert_eq!(*rcu.read(), "bar");
    /// ```
    pub fn write(&self, new_value: Arc<T>) {
        let new_ptr = Arc::into_raw(new_value) as *mut _;
        let old_ptr = self.ptr.swap(new_ptr, Ordering::Release);

        // Decrement the reference count of the inner Arc<T>
        unsafe {
            drop(Arc::from_raw(old_ptr));
        }
    }
}

impl<T: Default> Default for Rcu<T> {
    /// Creates a new `Rcu<T>`, with the `Default` value for T.
    fn default() -> Self {
        Self::new(Arc::new(T::default()))
    }
}

impl<T> From<T> for Rcu<T> {
    /// Creates a new `Rcu<T>` from T.
    fn from(value: T) -> Self {
        Self::new(Arc::new(value))
    }
}

impl<T: fmt::Debug> fmt::Debug for Rcu<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut d = f.debug_struct("Rcu");
        d.field("data", &self.read());
        d.finish_non_exhaustive()
    }
}

/// These tests make sure dropping is predictable and that all versions get dropped
#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Mutex};

    use super::*;

    #[derive(Debug, PartialEq, Eq)]
    enum Event {
        Initialize(usize),
        Clone { from: usize, to: usize },
        Drop(usize),
    }

    #[derive(Clone, Default, Debug)]
    struct Events(Arc<Mutex<(Vec<Event>, usize)>>);
    impl Events {
        fn next_version_number(&self) -> usize {
            let mut inner = self.0.lock().unwrap();
            let version_number = inner.1;
            inner.1 += 1;
            version_number
        }
        fn push(&self, event: Event) {
            self.0.lock().unwrap().0.push(event)
        }
        #[track_caller]
        fn assert_all_are_dropped(&self) {
            let mut living_versions = HashSet::new();

            let inner = self.0.lock().unwrap();
            for event in inner.0.iter() {
                match event {
                    Event::Initialize(version) => living_versions.insert(version),
                    Event::Clone { from: _, to } => living_versions.insert(to),
                    Event::Drop(version) => living_versions.remove(&version),
                };
            }

            assert!(
                living_versions.is_empty(),
                "Still living: {:?}",
                living_versions
            );
        }
    }

    struct Version {
        events: Events,
        version_number: usize,
        data: &'static str,
    }
    impl Version {
        fn new(events: Events, data: &'static str) -> Self {
            let version_number = events.next_version_number();
            events.push(Event::Initialize(version_number));

            Self {
                events,
                version_number,
                data,
            }
        }
    }
    impl Drop for Version {
        fn drop(&mut self) {
            if std::thread::panicking() {
                // Would otherwise also panic about a poisoning error
                return;
            }
            let mut events = self.events.0.lock().unwrap();

            events.0.push(Event::Drop(self.version_number));
        }
    }
    impl Clone for Version {
        fn clone(&self) -> Self {
            let new_version_number = self.events.next_version_number();
            self.events.push(Event::Clone {
                from: self.version_number,
                to: new_version_number,
            });
            Self {
                events: self.events.clone(),
                version_number: new_version_number,
                data: self.data,
            }
        }
    }

    #[test]
    fn test_empty_events() {
        let events = Events::default();

        assert_eq!(events.0.lock().unwrap().0, vec![]);
    }

    #[test]
    fn test_simple() {
        let events = Events::default();

        let rcu = Rcu::new(Arc::new(Version::new(events.clone(), "first version")));

        rcu.write(Arc::new(Version::new(events.clone(), "second version")));

        drop(rcu);

        assert_eq!(
            events.0.lock().unwrap().0,
            vec![
                Event::Initialize(0),
                Event::Initialize(1),
                Event::Drop(0),
                Event::Drop(1)
            ]
        );
        events.assert_all_are_dropped();
    }

    #[test]
    fn test_read() {
        let events = Events::default();

        let rcu = Rcu::new(Arc::new(Version::new(events.clone(), "first version")));

        let first_ver = rcu.read();

        rcu.write(Arc::new(Version::new(events.clone(), "second version")));

        drop(rcu);
        drop(first_ver);

        assert_eq!(
            events.0.lock().unwrap().0,
            vec![
                Event::Initialize(0),
                Event::Initialize(1),
                Event::Drop(1),
                Event::Drop(0),
            ]
        );
        events.assert_all_are_dropped();
    }

    #[test]
    fn test_update() {
        let events = Events::default();

        let rcu = Rcu::new(Arc::new(Version::new(events.clone(), "first version")));

        rcu.update(|version| version.data = "modified first version");

        drop(rcu);

        assert_eq!(
            events.0.lock().unwrap().0,
            vec![
                Event::Initialize(0),
                Event::Clone { from: 0, to: 1 },
                Event::Drop(0),
                Event::Drop(1),
            ]
        );
        events.assert_all_are_dropped();
    }

    #[test]
    fn test_multiple() {
        let events = Events::default();

        let rcu1 = Arc::new(Rcu::new(Arc::new(Version::new(
            events.clone(),
            "first version",
        ))));
        let rcu2 = rcu1.clone();

        rcu2.write(Arc::new(Version::new(events.clone(), "second version")));

        drop(rcu1);

        rcu2.write(Arc::new(Version::new(events.clone(), "third version")));

        drop(rcu2);

        assert_eq!(
            events.0.lock().unwrap().0,
            vec![
                Event::Initialize(0),
                Event::Initialize(1),
                Event::Drop(0),
                Event::Initialize(2),
                Event::Drop(1),
                Event::Drop(2),
            ]
        );
        events.assert_all_are_dropped();
    }

    #[test]
    fn test_multiple_threads() {
        let events = Events::default();

        let rcu1 = Arc::new(Rcu::new(Arc::new(Version::new(
            events.clone(),
            "first version",
        ))));

        let events2 = events.clone();
        let rcu2 = rcu1.clone();
        std::thread::spawn(move || {
            rcu2.write(Arc::new(Version::new(events2, "second version")));
        })
        .join()
        .unwrap();

        rcu1.write(Arc::new(Version::new(events.clone(), "third version")));

        drop(rcu1);

        assert_eq!(
            events.0.lock().unwrap().0,
            vec![
                Event::Initialize(0),
                Event::Initialize(1),
                Event::Drop(0),
                Event::Initialize(2),
                Event::Drop(1),
                Event::Drop(2),
            ]
        );
        events.assert_all_are_dropped();
    }
}
