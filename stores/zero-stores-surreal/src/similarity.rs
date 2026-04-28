//! Cosine-similarity helper shared between in-Rust scorers (episodes,
//! procedures, wiki). Intentionally tiny — there's no need to pull in a
//! linalg crate for a 384-dimensional dot product.
//!
//! Returns `None` if the vectors mismatch in length or either is empty.

pub fn cosine(a: &[f32], b: &[f32]) -> Option<f64> {
    if a.len() != b.len() || a.is_empty() {
        return None;
    }
    let mut dot = 0f64;
    let mut na = 0f64;
    let mut nb = 0f64;
    for (x, y) in a.iter().zip(b.iter()) {
        let xf = *x as f64;
        let yf = *y as f64;
        dot += xf * yf;
        na += xf * xf;
        nb += yf * yf;
    }
    let denom = (na.sqrt() * nb.sqrt()).max(1e-12);
    Some(dot / denom)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identical_vectors_score_one() {
        let v = vec![0.6_f32, 0.8];
        let s = cosine(&v, &v).unwrap();
        assert!((s - 1.0).abs() < 1e-6);
    }

    #[test]
    fn orthogonal_vectors_score_zero() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let s = cosine(&a, &b).unwrap();
        assert!(s.abs() < 1e-6);
    }

    #[test]
    fn mismatched_lengths_return_none() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![1.0_f32];
        assert!(cosine(&a, &b).is_none());
    }
}
