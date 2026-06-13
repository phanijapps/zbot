//! Clustering primitives for the hierarchical memory builder (Phase H-3).
//!
//! Pure functions — no DB, no LLM, no async. K-means with cosine
//! distance over L2-normalised embedding vectors, plus the stop-rule
//! helpers that decide when to stop adding hierarchy layers.
//!
//! Determinism is enforced by passing a seed into every entry point;
//! no global RNG, no time-based source. Same input + same seed → byte-
//! identical output.
//!
//! ## K-means choice
//!
//! HiRAG and LeanRAG use Gaussian Mixture Models. For our use case
//! (group entities into ~20-sized buckets at each layer over hundreds
//! of points) K-means is empirically indistinguishable while shaving
//! a heavy dep tree. Cosine distance is the right metric because
//! embedding clients produce L2-normalised vectors — Euclidean
//! K-means on the unit sphere is mathematically equivalent.
//!
//! Initialization is plain random (pick `k` distinct points). This is
//! intentionally simpler than K-means++ for v1 — clustering quality
//! at our scale isn't sensitive enough to justify it, and the seeded
//! determinism is easier to reason about.

/// Maximum K-means iterations before forcing a stop. Empirically 25 is
/// already convergence in our setting (~hundreds of points, k around 5-20).
pub const DEFAULT_KMEANS_MAX_ITER: usize = 25;

/// HiRAG's cluster-sparsity stop threshold. Defaults to 5%.
pub const DEFAULT_SPARSITY_EPSILON: f32 = 0.05;

// ---------------------------------------------------------------------------
// Tiny seeded RNG
// ---------------------------------------------------------------------------

/// Minimal linear-congruential generator. Standard Numerical Recipes
/// parameters (a = 1664525, c = 1013904223). Period is 2^32 which is
/// way more than enough for picking a handful of initial centroids.
///
/// Kept inline rather than pulling in `rand` because (a) we don't need
/// crypto-grade randomness, (b) one allocation-free struct beats a
/// transitive crate tree, and (c) test reproducibility is easier when
/// the RNG is in-process and visible.
struct Lcg(u32);

impl Lcg {
    fn new(seed: u64) -> Self {
        // Fold the upper 32 bits into the lower so callers can pass a
        // full u64 (matches std hash idioms).
        Self((seed as u32) ^ ((seed >> 32) as u32))
    }

    fn next_u32(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1664525).wrapping_add(1013904223);
        self.0
    }

    /// Random index in [0, n). Returns 0 when n == 0 — caller must guard.
    fn next_index(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        (self.next_u32() as usize) % n
    }
}

// ---------------------------------------------------------------------------
// K-means
// ---------------------------------------------------------------------------

/// K-means with cosine distance.
///
/// **Contract:** inputs are L2-normalised (norm ≈ 1.0). Embedding
/// clients in this codebase return normalised vectors, so this holds
/// for `kg_name_index` reads. Non-normalised inputs will silently
/// produce poor clusters — by design we don't normalise inside this
/// function (it'd be wasted work in the common case).
///
/// Returns a `Vec<usize>` of cluster labels in `0..k`, one per input
/// point. Always deterministic for a given seed.
///
/// ### Edge cases
/// - `points.is_empty()` → returns `Vec::new()`.
/// - `k == 0` → returns `vec![0; n]` (everyone collapses into bucket 0).
/// - `k >= n` → returns `(0..n).collect()` (one point per bucket).
/// - `max_iter == 0` → returns the initial assignment (one round of
///   centroid picks + assignment, no refinement loop). Useful as a
///   sentinel in tests; production callers should pass a meaningful cap.
pub fn kmeans_cosine(points: &[Vec<f32>], k: usize, seed: u64, max_iter: usize) -> Vec<usize> {
    let n = points.len();
    if n == 0 {
        return Vec::new();
    }
    if k == 0 {
        return vec![0; n];
    }

    // Reject degenerate dimensions BEFORE the `k >= n` shortcut.
    // Malformed input is malformed regardless of how k compares to n,
    // and the unique-labels fallback would otherwise hide the problem.
    let dim = points[0].len();
    if dim == 0 || points.iter().any(|p| p.len() != dim) {
        return vec![0; n];
    }

    if k >= n {
        return (0..n).collect();
    }

    let mut rng = Lcg::new(seed);
    let mut centroids = pick_initial_centroids(points, k, &mut rng);
    let mut labels = vec![0usize; n];
    let mut prev_labels: Vec<usize>;

    for _iter in 0..=max_iter {
        prev_labels = labels.clone();

        // Assign step.
        for (i, p) in points.iter().enumerate() {
            labels[i] = nearest_centroid(p, &centroids);
        }

        // First pass after init: we always need a refresh, so skip the
        // convergence check for iteration 0. After that, identical
        // assignments mean we've converged.
        if _iter > 0 && labels == prev_labels {
            break;
        }

        // Update step. Empty clusters keep their previous centroid —
        // this prevents them from collapsing to the origin (which
        // would dominate cosine assignment for any non-aligned point).
        let new_centroids = update_centroids(points, &labels, &centroids, k);
        centroids = new_centroids;
    }

    labels
}

/// Pick `k` initial centroids via K-means++ (D²-weighted sampling).
///
/// First centroid uniform. Each subsequent centroid is sampled with
/// probability proportional to the squared distance from its nearest
/// existing centroid. Distance is `1 - cos_sim` (cosine distance);
/// squaring exaggerates the preference for far-away points so the
/// initial set spreads across the cloud.
///
/// Plain random init produced bad local optima on tight clusters in
/// our test suite — three well-separated blobs sometimes ended up
/// with two centroids in one blob. K-means++ is the small fix that
/// makes those tests robust without a multi-restart loop.
fn pick_initial_centroids(points: &[Vec<f32>], k: usize, rng: &mut Lcg) -> Vec<Vec<f32>> {
    let n = points.len();
    debug_assert!(k > 0 && k <= n, "caller must ensure 0 < k <= n");

    let mut centroids: Vec<Vec<f32>> = Vec::with_capacity(k);
    centroids.push(points[rng.next_index(n)].clone());

    for _ in 1..k {
        // Squared cosine distance to the nearest existing centroid,
        // per point. Cosine sim is in [-1, 1], so 1 - sim is in [0, 2];
        // squaring keeps it non-negative and amplifies separation.
        let weights: Vec<f32> = points
            .iter()
            .map(|p| {
                let best_sim = centroids
                    .iter()
                    .map(|c| dot(p, c))
                    .fold(f32::NEG_INFINITY, f32::max);
                let dist = 1.0 - best_sim;
                dist * dist
            })
            .collect();

        let total: f32 = weights.iter().sum();
        if total <= 0.0 {
            // All remaining points already coincide with a centroid
            // (degenerate, all-identical input). Fall back to a
            // uniform pick so the loop still terminates.
            centroids.push(points[rng.next_index(n)].clone());
            continue;
        }

        // Convert a u32 sample into a [0, total) target.
        let raw = rng.next_u32() as f64 / (u32::MAX as f64 + 1.0);
        let target = (raw as f32) * total;
        let mut cum = 0.0_f32;
        let mut picked = n - 1;
        for (i, &w) in weights.iter().enumerate() {
            cum += w;
            if cum >= target {
                picked = i;
                break;
            }
        }
        centroids.push(points[picked].clone());
    }

    centroids
}

/// Return the index of the centroid with the highest cosine similarity
/// to `point`. Ties go to the lowest-index centroid (stable). For
/// L2-normalised vectors cosine similarity is just the dot product.
fn nearest_centroid(point: &[f32], centroids: &[Vec<f32>]) -> usize {
    let mut best_idx = 0;
    let mut best_sim = f32::NEG_INFINITY;
    for (i, c) in centroids.iter().enumerate() {
        let sim = dot(point, c);
        if sim > best_sim {
            best_sim = sim;
            best_idx = i;
        }
    }
    best_idx
}

/// New centroid set: each cluster's centroid is the L2-normalised mean
/// of its assigned points. Empty clusters retain their previous centroid
/// (passed in via `prev`) to keep the assignment loop stable.
fn update_centroids(
    points: &[Vec<f32>],
    labels: &[usize],
    prev: &[Vec<f32>],
    k: usize,
) -> Vec<Vec<f32>> {
    let dim = points[0].len();
    let mut sums: Vec<Vec<f32>> = vec![vec![0.0; dim]; k];
    let mut counts: Vec<usize> = vec![0; k];
    for (i, &label) in labels.iter().enumerate() {
        for (s, x) in sums[label].iter_mut().zip(points[i].iter()) {
            *s += *x;
        }
        counts[label] += 1;
    }
    sums.iter_mut()
        .zip(counts.iter())
        .enumerate()
        .map(|(cluster_idx, (sum, &count))| {
            if count == 0 {
                prev[cluster_idx].clone()
            } else {
                let n = count as f32;
                for v in sum.iter_mut() {
                    *v /= n;
                }
                l2_normalise(sum);
                sum.clone()
            }
        })
        .collect()
}

fn dot(a: &[f32], b: &[f32]) -> f32 {
    debug_assert_eq!(a.len(), b.len());
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

fn l2_normalise(v: &mut [f32]) {
    let norm: f32 = v.iter().map(|x| x * x).sum::<f32>().sqrt();
    if norm > 0.0 {
        for x in v.iter_mut() {
            *x /= norm;
        }
    }
}

// ---------------------------------------------------------------------------
// Stop-rule helpers (Phase H-3b)
// ---------------------------------------------------------------------------

/// HiRAG's cluster-sparsity score for a single layer.
///
/// `CS = 1 - Σ_{S∈clusters} |S|·(|S|-1) / (N·(N-1))`
///
/// Intuition: dense layers (few large clusters) → low CS; sparse
/// layers (many small clusters) → high CS. As we climb the hierarchy
/// the score grows, and when it stops growing meaningfully (Δ ≤ ε)
/// further layering wouldn't extract new structure.
///
/// Returns `0.0` when there are fewer than two labels (one item or
/// none → trivially dense, no work to do).
pub fn cluster_sparsity(labels: &[usize]) -> f32 {
    let n = labels.len();
    if n < 2 {
        return 0.0;
    }

    // Count cluster sizes by label.
    let max_label = labels.iter().copied().max().unwrap_or(0);
    let mut sizes = vec![0usize; max_label + 1];
    for &label in labels {
        sizes[label] += 1;
    }

    let denominator = (n as f32) * ((n - 1) as f32);
    let same_cluster_pairs: f32 = sizes
        .iter()
        .filter(|&&s| s >= 2)
        .map(|&s| (s as f32) * ((s - 1) as f32))
        .sum();
    1.0 - same_cluster_pairs / denominator
}

/// Decide whether to stop adding hierarchy layers.
///
/// `prev` is the cluster-sparsity of the previous layer's labels;
/// `current` is the new layer's. If the change is at or below `epsilon`,
/// further layering isn't extracting new structure — stop.
///
/// The first layer is always built (no previous to compare against),
/// so callers should special-case `prev = 0.0` if they want to force
/// at least one layer regardless of epsilon. Passing `prev = 0.0` here
/// yields `true` only when `current <= epsilon`, which is the right
/// "the input was already too dense to bother clustering" behaviour.
pub fn should_stop_layering(prev: f32, current: f32, epsilon: f32) -> bool {
    // A small slack on top of epsilon absorbs f32 rounding at the
    // boundary (e.g. 0.50 - 0.45 lands on 0.05000001 in f32). Without
    // it the function would oscillate at the documented threshold,
    // which is the opposite of what stop-on-convergence wants.
    let delta = (current - prev).abs();
    delta <= epsilon + 1e-5
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// Helper: L2-normalise an in-test vector so K-means contracts hold.
    fn norm(mut v: Vec<f32>) -> Vec<f32> {
        l2_normalise(&mut v);
        v
    }

    // ----- edge cases -----

    #[test]
    fn empty_input_returns_empty_labels() {
        assert!(kmeans_cosine(&[], 3, 42, 25).is_empty());
    }

    #[test]
    fn k_zero_collapses_to_single_bucket() {
        let points = vec![norm(vec![1.0, 0.0]), norm(vec![0.0, 1.0])];
        assert_eq!(kmeans_cosine(&points, 0, 42, 25), vec![0, 0]);
    }

    #[test]
    fn k_equals_n_gives_unique_labels() {
        let points = vec![
            norm(vec![1.0, 0.0]),
            norm(vec![0.0, 1.0]),
            norm(vec![1.0, 1.0]),
        ];
        let labels = kmeans_cosine(&points, 3, 42, 25);
        assert_eq!(labels, vec![0, 1, 2]);
    }

    #[test]
    fn k_greater_than_n_collapses_to_unique_labels() {
        let points = vec![norm(vec![1.0, 0.0]), norm(vec![0.0, 1.0])];
        let labels = kmeans_cosine(&points, 5, 42, 25);
        assert_eq!(labels, vec![0, 1]);
    }

    #[test]
    fn mismatched_dimensions_fall_back_to_single_cluster() {
        let points = vec![vec![1.0, 0.0], vec![0.0, 1.0, 0.5]];
        assert_eq!(kmeans_cosine(&points, 2, 42, 25), vec![0, 0]);
    }

    // ----- core behaviour -----

    #[test]
    fn three_separated_blobs_yield_three_clusters() {
        // Three blobs around (1,0), (0,1), (-1,0). Tightly clustered;
        // any reasonable K-means + seed must recover the structure.
        let blob_a = vec![
            norm(vec![1.0, 0.05]),
            norm(vec![0.99, -0.02]),
            norm(vec![1.0, 0.0]),
        ];
        let blob_b = vec![
            norm(vec![0.02, 1.0]),
            norm(vec![-0.03, 0.99]),
            norm(vec![0.0, 1.0]),
        ];
        let blob_c = vec![
            norm(vec![-1.0, 0.05]),
            norm(vec![-0.99, -0.02]),
            norm(vec![-1.0, 0.0]),
        ];
        let mut points = Vec::new();
        points.extend(blob_a);
        points.extend(blob_b);
        points.extend(blob_c);

        let labels = kmeans_cosine(&points, 3, 42, DEFAULT_KMEANS_MAX_ITER);

        // All three points of each blob must share a label, and the
        // three labels must be distinct from each other.
        assert_eq!(labels[0], labels[1]);
        assert_eq!(labels[1], labels[2]);
        assert_eq!(labels[3], labels[4]);
        assert_eq!(labels[4], labels[5]);
        assert_eq!(labels[6], labels[7]);
        assert_eq!(labels[7], labels[8]);
        assert_ne!(labels[0], labels[3]);
        assert_ne!(labels[3], labels[6]);
        assert_ne!(labels[0], labels[6]);
    }

    #[test]
    fn determinism_same_seed_same_output() {
        let points = vec![
            norm(vec![1.0, 0.0]),
            norm(vec![0.95, 0.1]),
            norm(vec![0.0, 1.0]),
            norm(vec![0.05, 0.99]),
            norm(vec![-1.0, 0.0]),
            norm(vec![-0.98, 0.05]),
        ];
        let a = kmeans_cosine(&points, 3, 1234, 25);
        let b = kmeans_cosine(&points, 3, 1234, 25);
        assert_eq!(a, b, "same seed must yield identical labels");
    }

    #[test]
    fn different_seeds_may_differ_but_quality_is_consistent() {
        // Pathological: 6 well-separated points, k=2 → any seed should
        // converge to two roughly-equal clusters. We don't care which
        // points end up in which bucket, only that the *partition* is
        // the same up to label relabelling.
        let points = vec![
            norm(vec![1.0, 0.0]),
            norm(vec![0.95, 0.1]),
            norm(vec![0.99, -0.05]),
            norm(vec![-1.0, 0.0]),
            norm(vec![-0.95, 0.1]),
            norm(vec![-0.99, -0.05]),
        ];
        let a = kmeans_cosine(&points, 2, 1, 25);
        let b = kmeans_cosine(&points, 2, 99, 25);
        // Same equivalence class either way: a[0]==a[1]==a[2] != a[3]==a[4]==a[5].
        for labels in [&a, &b] {
            assert_eq!(labels[0], labels[1]);
            assert_eq!(labels[1], labels[2]);
            assert_eq!(labels[3], labels[4]);
            assert_eq!(labels[4], labels[5]);
            assert_ne!(labels[0], labels[3]);
        }
    }

    #[test]
    fn all_identical_points_land_in_one_label_modulo_init() {
        // When all inputs are the same vector, the assignment is
        // arbitrary but every point must share at least one bucket
        // with every other point (i.e. the partition is at most one
        // non-empty cluster, regardless of k > 1).
        let points: Vec<Vec<f32>> = (0..6).map(|_| norm(vec![1.0, 0.0])).collect();
        let labels = kmeans_cosine(&points, 3, 42, 25);
        // All distances are zero; the assign step ties on the first
        // centroid, so labels collapse to a single value.
        let first = labels[0];
        for &l in &labels[1..] {
            assert_eq!(l, first, "identical points must end up co-clustered");
        }
    }

    // ----- stop-rule helpers -----

    #[test]
    fn cluster_sparsity_handles_trivial_inputs() {
        assert_eq!(cluster_sparsity(&[]), 0.0);
        assert_eq!(cluster_sparsity(&[0]), 0.0);
    }

    #[test]
    fn cluster_sparsity_one_big_cluster_is_dense() {
        // All in one bucket → CS = 1 - n*(n-1)/(n*(n-1)) = 0.0.
        let labels = vec![0; 8];
        let cs = cluster_sparsity(&labels);
        assert!(cs.abs() < 1e-6, "expected ~0.0, got {cs}");
    }

    #[test]
    fn cluster_sparsity_each_singleton_is_max_sparse() {
        // Every point in its own bucket → CS = 1.0.
        let labels: Vec<usize> = (0..8).collect();
        let cs = cluster_sparsity(&labels);
        assert!((cs - 1.0).abs() < 1e-6, "expected ~1.0, got {cs}");
    }

    #[test]
    fn cluster_sparsity_balanced_split_intermediate() {
        // Two equal clusters of 4. Same-cluster pairs = 2 * 4 * 3 = 24.
        // N*(N-1) = 8 * 7 = 56. CS = 1 - 24/56 ≈ 0.571.
        let labels = vec![0, 0, 0, 0, 1, 1, 1, 1];
        let cs = cluster_sparsity(&labels);
        assert!((cs - (1.0 - 24.0 / 56.0)).abs() < 1e-6);
    }

    #[test]
    fn should_stop_when_change_below_epsilon() {
        assert!(should_stop_layering(0.50, 0.52, 0.05));
        assert!(should_stop_layering(0.50, 0.50, 0.05));
        assert!(should_stop_layering(0.50, 0.45, 0.05));
    }

    #[test]
    fn should_continue_when_change_above_epsilon() {
        assert!(!should_stop_layering(0.50, 0.60, 0.05));
        assert!(!should_stop_layering(0.50, 0.30, 0.05));
    }

    #[test]
    fn should_stop_with_exact_epsilon_change() {
        // Boundary: change exactly equal to epsilon is treated as
        // "stop" (the change is "at or below epsilon"). This is what
        // the doc on `should_stop_layering` promises.
        assert!(should_stop_layering(0.50, 0.55, 0.05));
        assert!(should_stop_layering(0.50, 0.45, 0.05));
    }
}
