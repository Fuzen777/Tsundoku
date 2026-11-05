//! Locking shared state.

use std::sync::{Mutex, MutexGuard, PoisonError};

/// Locks `mutex`, even if a thread panicked while holding it. Every value the core shares this way stays consistent
/// through a panic: each change replaces it whole or is a single insert or remove.
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}
