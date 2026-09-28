#![allow(
    non_upper_case_globals,
    non_camel_case_types,
    non_snake_case,
    dead_code
)]

include!(concat!(env!("OUT_DIR"), "/bindings.rs"));

mod search;

use search::*;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::thread;
use std::time::Instant;

const NUM_THREADS: usize = 8;

fn main() {
    let cfg = Arc::new(SearchConfig::default());
    // ---- 在这里声明搜索条件 ----
    let searcher = Searcher::new(vec![
        Step {
            name: "hut".into(),
            origin: Origin::Spawn,
            radius: 1024,
            kind: StepKind::Structure(StructureType_Swamp_Hut),
        },
        Step {
            name: "mushroom".into(),
            origin: Origin::Named("hut".into()),
            radius: 128,
            kind: StepKind::Biome(BiomeID_mushroom_fields),
        },
    ]);

    let searcher = Arc::new(searcher);
    let found = Arc::new(AtomicU64::new(u64::MAX));
    let stop = Arc::new(AtomicBool::new(false));
    let counter = Arc::new(AtomicU64::new(0));

    let start = Instant::now();
    let mut handles = vec![];

    for tid in 0..NUM_THREADS {
        let searcher = searcher.clone();
        let found = found.clone();
        let stop = stop.clone();
        let counter = counter.clone();
        let cfg = cfg.clone();

        handles.push(thread::spawn(move || {
            let mut seed = tid as u64;
            let step = NUM_THREADS as u64;
            while !stop.load(Ordering::Relaxed) {
                counter.fetch_add(1, Ordering::Relaxed);

                if let Some(bindings) = searcher.matchs(&cfg, seed) {
                    if found
                        .compare_exchange(u64::MAX, seed, Ordering::SeqCst, Ordering::Relaxed)
                        .is_ok()
                    {
                        stop.store(true, Ordering::Relaxed);
                        println!("✅ 找到种子: {}", seed);
                        for (name, p) in &bindings {
                            println!("   {}: ({}, {})", name, p.0, p.1);
                        }
                    }
                    return;
                }
                seed = seed.wrapping_add(step);
            }
        }));
    }

    for h in handles {
        h.join().unwrap();
    }

    let elapsed = start.elapsed();
    println!(
        "耗时 {:.2?}，共检查 {} 个种子",
        elapsed,
        counter.load(Ordering::Relaxed)
    );

    let s = found.load(Ordering::Relaxed);
    if s != u64::MAX {
        println!("最终种子: {}", s);
    } else {
        println!("未找到");
    }
}
