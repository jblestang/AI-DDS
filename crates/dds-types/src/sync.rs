//! Poison-safe lock helpers.
//!
//! Mutex/RwLock poisoning must not abort the process in library code; recover the
//! guarded value and continue.

use std::sync::{Mutex, MutexGuard, PoisonError, RwLock, RwLockReadGuard, RwLockWriteGuard};

/// Lock a [`Mutex`], recovering from poison instead of panicking.
#[inline]
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Acquire a read lock, recovering from poison instead of panicking.
#[inline]
pub fn read<T>(rwlock: &RwLock<T>) -> RwLockReadGuard<'_, T> {
    rwlock.read().unwrap_or_else(PoisonError::into_inner)
}

/// Acquire a write lock, recovering from poison instead of panicking.
#[inline]
pub fn write<T>(rwlock: &RwLock<T>) -> RwLockWriteGuard<'_, T> {
    rwlock.write().unwrap_or_else(PoisonError::into_inner)
}
