use anyhow::{Result, anyhow};
use nalgebra::{DMatrix, DVector};
use ndarray::{Array1, Array2};

/// Least-squares solve of `a x = b` via one SVD reused across every `b`.
///
/// Rank-deficient `a` (teams that never separate across alliances) has no
/// identifiable solution, so it is rejected, not solved to arbitrary values.
pub fn svd(a: &Array2<f64>, bs: &[&Array1<f64>]) -> Result<Vec<Array1<f64>>> {
    let (rows, cols) = a.dim();
    if rows == 0 || cols == 0 {
        return Err(anyhow!("matrix A is empty"));
    }
    if rows < cols {
        return Err(anyhow!(
            "underdetermined: {rows} alliance rows for {cols} teams"
        ));
    }
    if bs.is_empty() {
        return Ok(vec![]);
    }
    if bs.iter().any(|b| b.len() != rows) {
        return Err(anyhow!("matrix dimensions do not align for least squares"));
    }

    let matrix = DMatrix::from_fn(rows, cols, |r, c| a[[r, c]]);
    let svd = matrix.svd(true, true);

    // Fewer than `cols` significant singular values = fit not identifiable.
    let s_max = svd.singular_values.iter().cloned().fold(0.0_f64, f64::max);
    let cutoff = s_max * 1e-9;
    if svd.singular_values.iter().filter(|&&s| s > cutoff).count() < cols {
        return Err(anyhow!(
            "rank-deficient system: teams are not separable from these alliances"
        ));
    }

    bs.iter()
        .map(|b| {
            let rhs = DVector::from_fn(rows, |r, _| b[r]);
            let x = svd
                .solve(&rhs, cutoff)
                .map_err(|e| anyhow!("least-squares solve failed: {e}"))?;
            Ok(Array1::from_iter(x.iter().copied()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Full rank: each of 6 teams alone, then 6 pairings.
    fn design() -> Array2<f64> {
        let pairs = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)];
        Array2::from_shape_fn((12, 6), |(r, c)| {
            if r < 6 {
                if r == c { 1.0 } else { 0.0 }
            } else {
                let (i, j) = pairs[r - 6];
                if c == i || c == j { 1.0 } else { 0.0 }
            }
        })
    }

    #[test]
    fn recovers_known_solution() {
        let a = design();
        let truth = Array1::from(vec![10.0, -3.5, 7.25, 0.0, 42.0, 1.75]);
        let b = a.dot(&truth);

        let x = svd(&a, &[&b]).unwrap();
        for (got, want) in x[0].iter().zip(truth.iter()) {
            assert!((got - want).abs() < 1e-9, "{got} vs {want}");
        }
    }

    #[test]
    fn solves_several_right_hand_sides_at_once() {
        let a = design();
        let truths = [
            Array1::from(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            Array1::from(vec![-1.0, 0.5, -2.0, 8.0, 0.0, 3.0]),
        ];
        let bs: Vec<Array1<f64>> = truths.iter().map(|t| a.dot(t)).collect();
        let refs: Vec<&Array1<f64>> = bs.iter().collect();

        let x = svd(&a, &refs).unwrap();
        assert_eq!(x.len(), 2);
        for (col, truth) in x.iter().zip(&truths) {
            for (got, want) in col.iter().zip(truth.iter()) {
                assert!((got - want).abs() < 1e-9, "{got} vs {want}");
            }
        }
    }

    #[test]
    fn rejects_mismatched_rhs() {
        let a = design();
        let good = Array1::zeros(12);
        let bad = Array1::zeros(11);
        assert!(svd(&a, &[&good, &bad]).is_err());
    }

    #[test]
    fn rejects_underdetermined() {
        let a = Array2::from_elem((4, 5), 1.0);
        let b = Array1::from_elem(4, 10.0);
        assert!(svd(&a, &[&b]).is_err());
    }

    #[test]
    fn rejects_rank_deficient_instead_of_returning_garbage() {
        // Two teams always paired: identical columns, not separable — must error.
        let a = Array2::from_shape_fn((6, 2), |(_, _)| 1.0);
        let b = Array1::from_elem(6, 50.0);
        let err = svd(&a, &[&b]).unwrap_err().to_string();
        assert!(err.contains("rank-deficient"), "{err}");
    }

    #[test]
    fn accepts_square_full_rank_system() {
        let a = Array2::from_shape_fn((5, 5), |(r, c)| if r == c { 2.0 } else { 0.0 });
        let b = Array1::from_elem(5, 4.0);
        let x = svd(&a, &[&b]).unwrap();
        assert!(x[0].iter().all(|v| (v - 2.0).abs() < 1e-9));
    }

    #[test]
    fn empty_rhs_list_is_empty() {
        assert!(svd(&design(), &[]).unwrap().is_empty());
    }
}
