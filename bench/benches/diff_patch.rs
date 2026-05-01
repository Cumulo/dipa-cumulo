/// Rust dipa diff/patch benchmark using typed structs + binary delta encoding.
/// Mirrors the dipa-cumulo server/client pipeline:
///   diff  : base.create_delta_towards(&new)  +  postcard::to_allocvec(delta.delta)
///   patch : postcard::from_bytes::<DeltaOwned>(&bytes)  +  state.apply_patch(delta)
///
/// This is the same algorithm class as MoonBit recollect (diff_value / apply_to_value)
/// and Calcit recollect (diff-twig / patch-twig).
use bench::{load_fixture, ChatState};
use criterion::{criterion_group, criterion_main, Criterion};
use cumulo_dipa::{Diffable, Patchable};
use std::hint::black_box;

/// Type alias for the owned delta produced by dipa for ChatState.
/// Matches the pattern in dipa-cumulo/client/src/lib.rs.
type ChatStateDelta = <ChatState as Diffable<'static, 'static, ChatState>>::DeltaOwned;

fn do_diff(base: &ChatState, new_state: &ChatState) -> Vec<u8> {
    let created = base.create_delta_towards(new_state);
    postcard::to_allocvec(&created.delta).unwrap()
}

fn do_patch(base: &ChatState, patch_bytes: &[u8]) -> ChatState {
    let delta: ChatStateDelta = postcard::from_bytes(patch_bytes).unwrap();
    let mut out = base.clone();
    out.apply_patch(delta);
    out
}

fn bench_diff_patch(c: &mut Criterion) {
    // ── Load and deserialize fixtures once ───────────────────────────────────
    let base: ChatState = load_fixture("state_base.json");
    let single_msg: ChatState = load_fixture("state_single_msg.json");
    let bulk_status: ChatState = load_fixture("state_bulk_status.json");
    let new_thread: ChatState = load_fixture("state_new_thread.json");
    let reorder: ChatState = load_fixture("state_reorder.json");

    println!("Fixtures loaded (struct deserialization done).");

    // ── Pre-compute serialized deltas (outside hot loop) ─────────────────────
    let patch_single = do_diff(&base, &single_msg);
    let patch_bulk = do_diff(&base, &bulk_status);
    let patch_thread = do_diff(&base, &new_thread);
    let patch_reorder = do_diff(&base, &reorder);

    println!("Serialized delta sizes (bytes):");
    println!("  single_msg  : {}", patch_single.len());
    println!("  bulk_status : {}", patch_bulk.len());
    println!("  new_thread  : {}", patch_thread.len());
    println!("  reorder     : {}", patch_reorder.len());

    // ── diff benchmarks ───────────────────────────────────────────────────────
    let mut diff_group = c.benchmark_group("diff");

    diff_group.bench_function("single_msg", |b| {
        b.iter(|| black_box(do_diff(black_box(&base), black_box(&single_msg))))
    });
    diff_group.bench_function("bulk_status", |b| {
        b.iter(|| black_box(do_diff(black_box(&base), black_box(&bulk_status))))
    });
    diff_group.bench_function("new_thread", |b| {
        b.iter(|| black_box(do_diff(black_box(&base), black_box(&new_thread))))
    });
    diff_group.bench_function("reorder", |b| {
        b.iter(|| black_box(do_diff(black_box(&base), black_box(&reorder))))
    });

    diff_group.finish();

    // ── patch benchmarks ──────────────────────────────────────────────────────
    let mut patch_group = c.benchmark_group("patch");

    patch_group.bench_function("single_msg", |b| {
        b.iter(|| black_box(do_patch(black_box(&base), black_box(&patch_single))))
    });
    patch_group.bench_function("bulk_status", |b| {
        b.iter(|| black_box(do_patch(black_box(&base), black_box(&patch_bulk))))
    });
    patch_group.bench_function("new_thread", |b| {
        b.iter(|| black_box(do_patch(black_box(&base), black_box(&patch_thread))))
    });
    patch_group.bench_function("reorder", |b| {
        b.iter(|| black_box(do_patch(black_box(&base), black_box(&patch_reorder))))
    });

    patch_group.finish();

    // ── round-trip ────────────────────────────────────────────────────────────
    let mut rt_group = c.benchmark_group("round_trip");

    rt_group.bench_function("single_msg", |b| {
        b.iter(|| {
            let p = do_diff(black_box(&base), black_box(&single_msg));
            black_box(do_patch(black_box(&base), &p))
        })
    });
    rt_group.bench_function("bulk_status", |b| {
        b.iter(|| {
            let p = do_diff(black_box(&base), black_box(&bulk_status));
            black_box(do_patch(black_box(&base), &p))
        })
    });
    rt_group.bench_function("new_thread", |b| {
        b.iter(|| {
            let p = do_diff(black_box(&base), black_box(&new_thread));
            black_box(do_patch(black_box(&base), &p))
        })
    });
    rt_group.bench_function("reorder", |b| {
        b.iter(|| {
            let p = do_diff(black_box(&base), black_box(&reorder));
            black_box(do_patch(black_box(&base), &p))
        })
    });

    rt_group.finish();
}

criterion_group!(benches, bench_diff_patch);
criterion_main!(benches);

