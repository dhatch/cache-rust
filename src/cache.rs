use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::fmt;
use std::mem;
use std::collections::HashMap;
use intrusive_collections::{LinkedList, LinkedListLink};
use intrusive_collections::intrusive_adapter;

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


pub struct LRUCache<K: Eq + std::hash::Hash + Copy, V> {
    map: HashMap<K, Rc<CacheValue<K, V>>>,
    lru_list: RefCell<LinkedList<CacheValueAdapter<K, V>>>,
    capacity: usize
}


impl <K: Eq + std::hash::Hash + Copy, V> LRUCache<K, V> {
    /// Create a LRUCache with space for ``capacity`` items.
    pub fn new(capacity: usize) -> LRUCache<K, V> {
        LRUCache {
            map: HashMap::with_capacity(capacity),
            lru_list: RefCell::new(LinkedList::new(CacheValueAdapter::new())),
            capacity
        }
    }

    /// Return the value in the cache for ``key``, if it exists.
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.map.get(key) {
            None => None,
            Some(cache_value) => {
                self.touch(cache_value);
                Some(&cache_value.value)
            }
        }
    }

    /// Put ``value`` into cache for ``key``.
    /// Returns the previous value if there was one.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        let cache_value = Rc::new(CacheValue::new(key, value));
        if !self.map.contains_key(&key) {
            self.make_room();
        }

        let old_value = match self.map.insert(key, Rc::clone(&cache_value)) {
            None => None,
            Some(cache_value) => {
                let value;

                unsafe {
                    let raw = Rc::into_raw(cache_value);
                    let mut cursor = self.lru_list.get_mut().cursor_mut_from_ptr(raw);
                    value = cursor.remove();
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

        self.lru_list.get_mut().push_front(Rc::clone(&cache_value));

        old_value
    }

    /// Update access tracking, indicating that a cache value has been accessed.
    ///
    /// Safety Note:
    ///   - Assumes that ``cache_value`` is already in lru_list.  If not, behavior is
    ///     undefined.
    fn touch(&self, cache_value: &CacheValue<K, V>) {
        let mut lru_list = self.lru_list.borrow_mut();

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
        if self.map.len() == self.capacity {
            self.evict_lru();
        }
    }

    /// Perform lru eviction.
    fn evict_lru(&mut self) {
        let lru_value = self.lru_list.get_mut().pop_front();
        if let None = self.map.remove(&lru_value.expect("List must not be none").key) {
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
        assert_eq!(cache.get(&k1), Some(&2));
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
        assert_eq!(cache.map.len(), 0);

        cache.put(k1, v1);
        assert_eq!(cache.map.len(), 1);

        cache.put(k2, v2);
        assert_eq!(cache.map.len(), 1);

        assert_eq!(cache.get(&k1), None);
    }

    #[test]
    fn replace() {
        let k1 = "key1";
        let k2 = "key2";
        let v1 = 1;
        let v2 = 2;

        let mut cache: LRUCache<&str, u64> = LRUCache::new(1);
        assert_eq!(cache.map.len(), 0);

        cache.put(k1, v1);
        cache.put(k1, v2);
        assert_eq!(cache.map.len(), 1);
    }
}
