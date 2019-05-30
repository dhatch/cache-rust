extern crate cache;
#[macro_use]
extern crate bencher;

use bencher::Bencher;
use cache::cache::LRUCache;

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

benchmark_group!(benches, bench_insert, bench_read);
benchmark_main!(benches);
