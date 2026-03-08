use anyhow::{Result, anyhow};
use faer::{Col, Mat};
use faer::prelude::SpSolverLstsq;
use ndarray::{Array1, Array2};

#[derive(Debug, Clone)]
pub struct MetricSolution {
    pub auto: Array1<f64>,
    pub tele: Array1<f64>,
    pub endgame: Array1<f64>,
    pub penalties: Array1<f64>,
}

pub fn lse(a: &Array2<f64>, b: &Array1<f64>) -> Result<Array1<f64>> {
    let (rows, cols) = a.dim();
    if rows == 0 || cols == 0 {
        return Err(anyhow!("matrix A is empty"));
    }
    if b.len() != rows {
        return Err(anyhow!("matrix dimensions do not align for least squares"));
    }

    let a_faer = Mat::from_fn(rows, cols, |r, c| a[[r, c]]);
    let b_faer = Col::from_fn(rows, |r| b[r]);
    let x = a_faer.col_piv_qr().solve_lstsq(&b_faer);

    Ok(Array1::from_iter(x.iter().copied()))
}

pub fn solve_metrics(
    a: &Array2<i32>,
    auto: &Array1<i32>,
    tele: &Array1<i32>,
    endgame: &Array1<i32>,
    penalties: &Array1<i32>,
) -> Result<MetricSolution> {
    let a_f = a.mapv(|v| v as f64);

    Ok(MetricSolution {
        auto: lse(&a_f, &auto.mapv(|v| v as f64))?,
        tele: lse(&a_f, &tele.mapv(|v| v as f64))?,
        endgame: lse(&a_f, &endgame.mapv(|v| v as f64))?,
        penalties: lse(&a_f, &penalties.mapv(|v| v as f64))?,
    })
}
