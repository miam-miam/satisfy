use std::any::TypeId;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::sync::{Mutex, OnceLock, PoisonError};

/// Append-only, build-once map.
///
/// Provides a `get_or_init` that
/// can ensure single initialisation of values into a hashmap
/// whilst providing a reference to the value.
pub(crate) struct OnceMap<K, V> {
    // This map must remain append-only, because `get_or_init`
    // returns a reference to the value. If we ever removed a key,
    // that reference would become dangling.
    map: Mutex<HashMap<K, Box<Cell<V>>>>,
}

struct Cell<V> {
    value: OnceLock<V>,
    // Serialises fallible builds, which `OnceLock` can't run on stable.
    try_init: Mutex<()>,
}

impl<V> Cell<V> {
    fn new(value: OnceLock<V>) -> Self {
        Cell {
            value,
            try_init: Mutex::new(()),
        }
    }
}

impl<K: Eq + Hash + Copy, V> OnceMap<K, V> {
    pub(crate) fn new() -> Self {
        OnceMap {
            map: Mutex::default(),
        }
    }

    fn cell(&self, key: K) -> &Cell<V> {
        let ptr = &**self
            .map
            .lock()
            .unwrap()
            .entry(key)
            .or_insert_with(|| Box::new(Cell::new(OnceLock::new())))
            as *const Cell<V>;
        // SAFETY: the Box is never dropped or replaced while &self is live (see map invariant).
        unsafe { &*ptr }
    }

    /// Returns the value for `key`, running `create` if there is none yet.
    ///
    /// Racing callers for the same key wait for the first `create` rather than running their own.
    ///
    /// `create` may call back into this map for other keys but may not call back into the same key.
    pub(crate) fn get_or_init(&self, key: K, create: impl FnOnce() -> V) -> &V {
        self.cell(key).value.get_or_init(create)
    }

    /// Like [`get_or_init`](Self::get_or_init), but an `Err` from `create` is returned and
    /// nothing is stored, so the next call runs `create` again.
    pub(crate) fn get_or_try_init<E>(
        &self,
        key: K,
        create: impl FnOnce() -> Result<V, E>,
    ) -> Result<&V, E> {
        let cell = self.cell(key);
        if let Some(value) = cell.value.get() {
            return Ok(value);
        }
        // The guard protects no data, so a panic in another `create` leaves nothing to repair.
        let _guard = cell.try_init.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(value) = cell.value.get() {
            return Ok(value);
        }
        let value = create()?;
        Ok(cell.value.get_or_init(|| value))
    }

    /// Sets or replaces the value for `key`.
    pub(crate) fn set(&mut self, key: K, value: V) {
        self.map
            .get_mut()
            .unwrap()
            .insert(key, Box::new(Cell::new(OnceLock::from(value))));
    }

    /// Keys whose value has finished initialising; ones still being created are left out.
    pub(crate) fn initialised_keys(&self) -> Vec<K> {
        let map = self.map.lock().unwrap();
        map.iter()
            .filter(|(_, cell)| cell.value.get().is_some())
            .map(|(k, _)| *k)
            .collect()
    }
}

// Carries the type's name so `Debug` can list the type-erased entries.
#[derive(Clone, Copy)]
pub(crate) struct TypeKey {
    id: TypeId,
    pub(crate) name: &'static str,
}

impl TypeKey {
    pub(crate) fn of<T: ?Sized + 'static>() -> Self {
        TypeKey {
            id: TypeId::of::<T>(),
            name: std::any::type_name::<T>(),
        }
    }
}

impl PartialEq for TypeKey {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for TypeKey {}

impl Hash for TypeKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl fmt::Debug for TypeKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name)
    }
}

// Hash maps iterate in a different order on every run, so `Debug` output sorts their keys.
pub(crate) fn sorted_by_name(mut keys: Vec<TypeKey>) -> Vec<TypeKey> {
    keys.sort_by_key(|k| k.name);
    keys
}

#[cfg(test)]
mod tests {
    use super::OnceMap;
    use std::sync::Barrier;
    use std::sync::atomic::{AtomicU32, Ordering};
    use std::thread;
    use std::time::Duration;

    #[test]
    fn racing_try_inits_build_once() {
        let map = OnceMap::<u8, u32>::new();
        let builds = AtomicU32::new(0);
        let start = Barrier::new(8);

        thread::scope(|s| {
            for _ in 0..8 {
                s.spawn(|| {
                    start.wait();
                    let value = map.get_or_try_init(1, || {
                        builds.fetch_add(1, Ordering::Relaxed);
                        thread::sleep(Duration::from_millis(20));
                        Ok::<_, ()>(7)
                    });
                    assert_eq!(value, Ok(&7));
                });
            }
        });

        assert_eq!(builds.load(Ordering::Relaxed), 1);
    }
}
