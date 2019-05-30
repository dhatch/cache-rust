extern crate cache;
#[macro_use]
extern crate bencher;
extern crate rand;

use std::thread;
use std::sync::Arc;
use bencher::Bencher;
use cache::cache::LRUCache;
use rand::prelude::*;

fn bench_insert(b: &mut Bencher) {
    let mut cache: LRUCache<u64, u64> = LRUCache::new(128);
    let mut idx = 0;
    b.iter(|| {
        cache.put(idx, idx);
        idx += 1;
    });
}

fn bench_read(b: &mut Bencher) {
    let mut cache: LRUCache<u64, u64> = LRUCache::new(4096);
    let mut idx = 0;

    for idx in 0..4096 {
        cache.put(idx, idx);
    }

    b.iter(|| {
        cache.get(&idx);
        idx += 1;
    });
}

fn bench_threads(b: &mut Bencher) {
    let cap = 128;
    let mut cache: LRUCache<u64, u64> = LRUCache::new(cap);

    for idx in 0..cap {
        cache.put(idx as u64, idx as u64);
    }

    let cache = Arc::new(cache);
    b.iter(|| {

        let cacheA = Arc::clone(&cache);
        let thread1 = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            for _ in 0..1000 {
                let val: u64 = rng.gen();
                cacheA.get(&val);
            }
        });

        let cacheB = Arc::clone(&cache);
        let thread2 = thread::spawn(move || {
            let mut rng = rand::thread_rng();
            for _ in 0..1000 {
                let val: u64 = rng.gen();
                cacheB.get(&val);
            }
        });

        thread1.join().unwrap();
        thread2.join().unwrap();
    })
}


benchmark_group!(benches, bench_insert, bench_read, bench_threads);
benchmark_main!(benches);
