use anyhow::{Result, anyhow, ensure};
use nalgebra::DMatrix;

/// Least-squares `a x = b`, one column of `x` per column of `b`. Rejects rank-deficient and
/// underdetermined systems.
pub fn least_squares(a: DMatrix<f64>, b: &DMatrix<f64>) -> Result<DMatrix<f64>> {
    let (rows, cols) = a.shape();
    ensure!(rows > 0 && cols > 0, "matrix A is empty");
    ensure!(
        rows >= cols,
        "underdetermined: {rows} alliance rows for {cols} teams"
    );
    ensure!(
        b.nrows() == rows,
        "matrix dimensions do not align for least squares"
    );

    let svd = a.svd(true, true);
    let cutoff = svd.singular_values.max() * 1e-9;
    ensure!(
        svd.rank(cutoff) == cols,
        "rank-deficient system: teams are not separable from these alliances"
    );

    svd.solve(b, cutoff)
        .map_err(|e| anyhow!("least-squares solve failed: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nalgebra::DVector;

    fn design() -> DMatrix<f64> {
        let pairs = [(0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 0)];
        DMatrix::from_fn(12, 6, |r, c| {
            let on = if r < 6 {
                r == c
            } else {
                let (i, j) = pairs[r - 6];
                c == i || c == j
            };
            if on { 1.0 } else { 0.0 }
        })
    }

    fn assert_close(got: &DMatrix<f64>, want: &DMatrix<f64>) {
        assert_eq!(got.shape(), want.shape());
        for (g, w) in got.iter().zip(want.iter()) {
            assert!((g - w).abs() < 1e-9, "{g} vs {w}");
        }
    }

    #[test]
    fn recovers_known_solution() {
        let a = design();
        let truth = DMatrix::from_column_slice(6, 1, &[10.0, -3.5, 7.25, 0.0, 42.0, 1.75]);
        let b = &a * &truth;
        assert_close(&least_squares(a, &b).unwrap(), &truth);
    }

    #[test]
    fn solves_several_right_hand_sides_at_once() {
        let a = design();
        let truth = DMatrix::from_columns(&[
            DVector::from_vec(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]),
            DVector::from_vec(vec![-1.0, 0.5, -2.0, 8.0, 0.0, 3.0]),
        ]);
        let b = &a * &truth;
        assert_close(&least_squares(a, &b).unwrap(), &truth);
    }

    #[test]
    fn rejects_mismatched_rhs() {
        assert!(least_squares(design(), &DMatrix::zeros(11, 1)).is_err());
    }

    #[test]
    fn rejects_underdetermined() {
        let a = DMatrix::from_element(4, 5, 1.0);
        assert!(least_squares(a, &DMatrix::from_element(4, 1, 10.0)).is_err());
    }

    #[test]
    fn rejects_rank_deficient_instead_of_returning_garbage() {
        let a = DMatrix::from_element(6, 2, 1.0);
        let err = least_squares(a, &DMatrix::from_element(6, 1, 50.0))
            .unwrap_err()
            .to_string();
        assert!(err.contains("rank-deficient"), "{err}");
    }

    #[test]
    fn accepts_square_full_rank_system() {
        let a = DMatrix::from_diagonal_element(5, 5, 2.0);
        let x = least_squares(a, &DMatrix::from_element(5, 1, 4.0)).unwrap();
        assert!(x.iter().all(|v| (v - 2.0).abs() < 1e-9));
    }
}
