use std::cell::Cell;
use std::collections::HashMap;
use intrusive_collections::{LinkedList, LinkedListLink};
use intrusive_collections::intrusive_adapter;

struct CacheValue<V> {
    value: V,
    link: LinkedListLink
}

impl <V> CacheValue<V> {
    fn new(value: V) -> CacheValue<V> {
        CacheValue {
            value,
            link: LinkedListLink::new()
        }
    }
}

intrusive_adapter!(CacheValueAdapter<'a, V> = &'a CacheValue<V>: CacheValue<V> { link: LinkedListLink });


pub struct LRUCache<'a, K: Eq + std::hash::Hash, V> {
    map: HashMap<K, CacheValue<V>>,
    lru_list: Cell<LinkedList<CacheValueAdapter<'a, V>>>,
    capacity: usize
}


impl <'a, K: Eq + std::hash::Hash, V> LRUCache<'a, K, V> {
    /// Create a LRUCache with space for ``capacity`` items.
    pub fn new(capacity: usize) -> LRUCache<'a, K, V> {
        LRUCache {
            map: HashMap::with_capacity(capacity),
            lru_list: Cell::new(LinkedList::new(CacheValueAdapter::new())),
            capacity
        }
    }

    /// Return the value in the cache for ``key``, if it exists.
    pub fn get(&self, key: &K) -> Option<&V> {
        match self.map.get(key) {
            None => None,
            Some(v) => {
                let cache_value = v;
                self.touch(cache_value);
                Some(&cache_value.value)
            }
        }
    }

    /// Put ``value`` into cache for ``key``.
    /// Returns the previous value if there was one.
    pub fn put(&mut self, key: K, value: V) -> Option<V> {
        let cache_value = CacheValue::new(value);
        let old_value = match self.map.insert(key, cache_value) {
            None => None,
            Some(v) => Some(v.value)
        };

        if let Some(v) = self.map.get(&key) {
            self.lru_list.get_mut().push_front(v);
        } else {
            unreachable!();
        }

        old_value
    }

    /// Update access tracking, indicating that a cache value has been accessed.
    ///
    /// Safety Note:
    ///   - Assumes that ``cache_value`` is already in lru_list.  If not, behavior is
    ///     undefined.
    fn touch(&self, cache_value: &'a CacheValue<V>) {
        let lru_list = self.lru_list.get_mut();
        let cursor = lru_list.cursor_mut_from_ptr(cache_value);
        cursor.remove();
        lru_list.push_front(cache_value)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hit() {
        let k1 = "key";
        let mut cache: LRUCache<&str, u64> = LRUCache::new(10);
        cache.put(k1, 2);
        assert_eq!(cache.get(&k1), Some(&2));
    }

    #[test]
    fn miss() {
        let k1 = "no key";
        let mut cache: LRUCache<&str, u64> = LRUCache::new(10);
        assert_eq!(cache.get(&k1), None);
    }
}
