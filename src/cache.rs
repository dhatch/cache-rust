use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::fmt;
use std::mem;
use std::sync::{Mutex, MutexGuard};
use std::collections::HashMap;
use intrusive_collections::{LinkedList, LinkedListLink};
use intrusive_collections::intrusive_adapter;

/// Wrapper for values in the cache that implements a node in a linked list.
struct CacheValue<K, V> {
    key: K,
    value: V,
    link: LinkedListLink
}

impl <K, V> CacheValue<K, V> {
    fn new(key: K, value: V) -> CacheValue<K, V> {
        CacheValue {
            key,
            value,
            link: LinkedListLink::new()
        }
    }
}

impl <K, V> fmt::Debug for CacheValue<K, V> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "CacheValue")
    }
}

intrusive_adapter!(CacheValueAdapter<K, V> = Rc<CacheValue<K, V>>: CacheValue<K, V> { link: LinkedListLink });


/// Not sure what is typical pattern in Rust, but using this to encapsulate data protected by a
/// mutex.
struct LRUCacheData<K, V> {
    map: HashMap<K, Rc<CacheValue<K, V>>>,
    lru_list: RefCell<LinkedList<CacheValueAdapter<K, V>>>
}

/// LRUCache implements an in-memory cache of fixed capacity with a least-recency-used replacement
/// policy.
///
/// The cache accepts any hashable and clonable value as a key type.
///
/// # Implementation Notes:
///
/// The LRUCache maintains a HashMap and doubly-linked-list to perform usage tracking.
///
/// Within both are reference-counted pointers to a CacheValue which implements an intrusive
/// linked list. The instrusive list is necessary so that the LRU position can be updated in O(1)
/// time (the linked list node is returned by the map.
///
/// The reference-counted pointers are required because Rust does not support self-referential
/// structs. (took me some time to realize this).
///
/// # Concurrency:
///
/// ...
///
/// # Alternative implementations:
///
/// ...
///
/// # Concerns:
///
/// This data structure is pretty poor for cache-locality (if I am understanding Rc correctly).
/// Each value is separately allocated, so the data the cache points to will not be brought into
/// cache together.  Ideally, we would allocate the memory that each Rc points to from a single
/// buffer.
pub struct LRUCache<K: Eq + std::hash::Hash + Clone, V: Clone> {
    data: Mutex<LRUCacheData<K, V>>,
    capacity: usize
}


impl <K: Eq + std::hash::Hash + Clone, V: Clone> LRUCache<K, V> {
    /// Create a LRUCache with space for `capacity` items.
    ///
    /// # Arguments:
    ///
    /// - `capacity`: The maximum number of items permitted in the cache.
    ///
    /// # NB:
    ///
    /// - The cache will allocate memory for all items, even if it is not full.
    pub fn new(capacity: usize) -> LRUCache<K, V> {
        LRUCache {
            data: Mutex::new(LRUCacheData {
                map: HashMap::with_capacity(capacity),
                lru_list: RefCell::new(LinkedList::new(CacheValueAdapter::new())),
            }),
            capacity
        }
    }

    /// Get the value for `key` in `self`, if it exists.  Otherwise, return `None`.
    ///
    /// Implementation note:
    ///
    /// The value is cloned out of the cache, otherwise the reader would need to keep
    /// the mutex while it works with the value. Since the cache is designed for servicing
    /// HTTP requests, a copy will be necessary anyway.
    ///
    /// This is an area for improvement (API to return some kind of handle that drops the mutex
    /// is too time consuming for me to figure out for now).
    pub fn get(&self, key: &K) -> Option<V> {
        let data = self.data.lock().unwrap();
        match data.map.get(key) {
            None => None,
            Some(cache_value) => {
                self.touch(&data, cache_value);
                Some(cache_value.value.clone())
            }
        }
    }

    /// Put `value` into `self` for `key`.
    ///
    /// # Returns
    ///
    /// The previous value in the cache, or `None`.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        let cache_value = Rc::new(CacheValue::new(key.clone(), value));

        // We only need to make room for a new value if we are not replacing an old one.
        // TODO: Ran into some borrow checker issues here that I couldn't figure out elegantly.
        // It seems like MutexGuard / mutable data cannot be safely passed around between
        // methods without releasing and reacquiring it.
        {
            let data = self.data.lock().unwrap();
            if !data.map.contains_key(&key) {
                mem::drop(data);
                self.make_room();
            }
        }

        let data = self.data.get_mut().unwrap();

        let old_value = match data.map.insert(key, Rc::clone(&cache_value)) {
            None => None,
            Some(cache_value) => {
                let value;

                // This unsafe block is required to remove the item from the intrusive linked list
                // contained in `CacheValue`.
                //
                // Assumes that cache_value is already in `lru_list`.
                unsafe {
                    let raw = Rc::into_raw(cache_value);
                    let mut cursor = data.lru_list.get_mut().cursor_mut_from_ptr(raw);
                    value = cursor.remove();

                    // Converts raw pointer back into a `Rc<CacheValue>` that can be dropped at the
                    // end of this scope.
                    Rc::from_raw(raw);
                }


                match Rc::try_unwrap(value.expect("Unexpected error")) {
                    Err(rc) => {
                        panic!("Expected one owner for rc, found {}", Rc::strong_count(&rc))
                    },
                    Ok(value) => {
                        Some(value.value)
                    }
                }
            }
        };

        data.lru_list.get_mut().push_front(Rc::clone(&cache_value));

        old_value
    }

    /// Update access tracking, indicating that a cache value has been accessed.
    ///
    /// Moves `cache_value` to the front of `lru_list`, indicating it has been used most recently.
    ///
    /// # Safety
    ///
    /// - Assumes that ``cache_value`` is already in lru_list.  If not, behavior is
    ///   undefined.
    fn touch(&self, data: &MutexGuard<LRUCacheData<K, V>>, cache_value: &CacheValue<K, V>) {
        let mut lru_list = data.lru_list.borrow_mut();

        let mut cursor;
        unsafe {
            cursor = lru_list.cursor_mut_from_ptr(cache_value);
        }

        if let Some(removed_value) = cursor.remove() {
            lru_list.push_front(removed_value);
        } else {
            unreachable!()
        }
    }

    /// Make room for a new value.  If the cache is full, perform eviction.
    fn make_room(&mut self) {
        let data = self.data.lock().unwrap();
        if data.map.len() == self.capacity {
            mem::drop(data);
            self.evict_lru();
        }
    }

    /// Perform lru eviction.
    fn evict_lru(&mut self) {
        let data = self.data.get_mut().unwrap();
        let lru_value = data.lru_list.get_mut().pop_front();
        if let None = data.map.remove(&lru_value.expect("List must not be none").key) {
            unreachable!();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit() {
        let k1 = "key";
        let mut cache: LRUCache<&str, u64> = LRUCache::new(1);
        cache.put(k1, 2);
        assert_eq!(cache.get(&k1), Some(2));
    }

    #[test]
    fn miss() {
        let k1 = "no key";
        let mut cache: LRUCache<&str, u64> = LRUCache::new(10);
        assert_eq!(cache.get(&k1), None);
    }

    #[test]
    fn evict() {
        let k1 = "key1";
        let k2 = "key2";
        let v1 = 1;
        let v2 = 2;

        let mut cache: LRUCache<&str, u64> = LRUCache::new(1);
        assert_eq!(cache.data.lock().unwrap().map.len(), 0);

        cache.put(k1, v1);
        assert_eq!(cache.data.lock().unwrap().map.len(), 1);

        cache.put(k2, v2);
        assert_eq!(cache.data.lock().unwrap().map.len(), 1);

        assert_eq!(cache.get(&k1), None);
    }

    #[test]
    fn replace() {
        let k1 = "key1";
        let k2 = "key2";
        let v1 = 1;
        let v2 = 2;

        let mut cache: LRUCache<&str, u64> = LRUCache::new(1);
        assert_eq!(cache.data.lock().unwrap().map.len(), 0);

        cache.put(k1, v1);
        cache.put(k1, v2);
        assert_eq!(cache.data.lock().unwrap().map.len(), 1);
    }
}
