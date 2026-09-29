//! Bounded strict-data probe. Timings include counting-allocator overhead.
//! Reports total allocation/reallocation requests and requested bytes, NOT RSS.
use march_research::fast::{
    Context,
    stack::{Machine, Value},
    store::Store,
    stream,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    hint::black_box,
    sync::atomic::{AtomicBool, AtomicUsize, Ordering::Relaxed},
    time::Instant,
};

struct Counting;
static ENABLED: AtomicBool = AtomicBool::new(false);
static ALLOCS: AtomicUsize = AtomicUsize::new(0);
static BYTES: AtomicUsize = AtomicUsize::new(0);
fn count(bytes: usize) {
    if ENABLED.load(Relaxed) {
        ALLOCS.fetch_add(1, Relaxed);
        BYTES.fetch_add(bytes, Relaxed);
    }
}
// SAFETY: Every allocation operation delegates unchanged to System. Counters
// do not allocate, and never modify allocation pointers/layouts or ownership.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc(layout) }
    }
    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        count(layout.size());
        unsafe { System.alloc_zeroed(layout) }
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }
    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        count(size);
        unsafe { System.realloc(ptr, layout, size) }
    }
}
#[global_allocator]
static ALLOCATOR: Counting = Counting;

fn main() {
    let iterations = std::env::args()
        .nth(1)
        .map(|s| s.parse::<usize>().unwrap())
        .unwrap_or(10000);
    assert!((1..=100000).contains(&iterations));
    let long = "x".repeat(65536);
    let cases = [
        (
            "text clone 16B",
            format!("\"{}\" dup drop text-bytes", "x".repeat(16)),
            16,
        ),
        (
            "text clone 64KiB",
            format!("\"{long}\" dup drop text-bytes"),
            65536,
        ),
        ("tuple construct", "1 2 3 4 tuple 4 tuple-length".into(), 4),
        (
            "tuple clone",
            "1 2 3 4 tuple 4 dup drop tuple-length".into(),
            4,
        ),
        (
            "tuple update",
            "1 2 3 4 tuple 4 2 9 tuple-set tuple-length".into(),
            4,
        ),
        (
            "nested store",
            "1 2 pair dup pair \"v\" store.put \"v\" store.get tuple-length".into(),
            2,
        ),
        (
            "text concat",
            "\"abcd\" \"efgh\" text-concat text-bytes".into(),
            8,
        ),
    ];
    println!(
        "{iterations} warm invocations/case; allocation counts include VM/store/temporary vectors"
    );
    for (name, source, expected) in cases {
        let mut p = stream::seed().unwrap();
        let w = stream::compile(&mut p, &source).unwrap();
        let mut vm = Machine::new(&p);
        let context = Context::new();
        let initial = Store::new();
        for _ in 0..100 {
            vm.run(w, &[], &context, 100000, &initial).unwrap();
        }
        ALLOCS.store(0, Relaxed);
        BYTES.store(0, Relaxed);
        ENABLED.store(true, Relaxed);
        let start = Instant::now();
        let mut checksum = 0i64;
        for _ in 0..iterations {
            let (v, s) = black_box(
                vm.run(black_box(w), &[], &context, 100000, &initial)
                    .unwrap(),
            );
            let [Value::Int(n)] = v.as_slice() else {
                panic!("unexpected result")
            };
            checksum += *n;
            black_box(s);
        }
        let ns = start.elapsed().as_nanos() as f64 / iterations as f64;
        ENABLED.store(false, Relaxed);
        assert_eq!(checksum, expected * iterations as i64);
        let a = ALLOCS.load(Relaxed) as f64 / iterations as f64;
        let b = BYTES.load(Relaxed) as f64 / iterations as f64;
        println!(
            "{name}: {ns:.1} ns/run, {a:.1} allocations/run, {b:.0} requested bytes/run; tuples={} fields={} new_text_bytes={} frozen_nodes={}",
            vm.stats().tuple_nodes,
            vm.stats().tuple_fields,
            vm.stats().text_bytes_allocated,
            vm.stats().frozen_nodes
        );
    }
}
