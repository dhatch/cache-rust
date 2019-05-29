extern crate intrusive_collections;

mod cache;

fn main() {
    let cache: cache::LRUCache<u64,u64> = cache::LRUCache::new(10);
    println!("Hello, world!");
}
