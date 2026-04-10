use nalgebra::{DMatrix, DVector, Vector3};
use rand::{Rng, SeedableRng};
use rand::rngs::StdRng;
use std::f64;
use rand::RngExt;

// ==========================================
// 1. Color Space Conversions (RGB <-> LAB)
// ==========================================

const D65_X: f64 = 0.95047;
const D65_Y: f64 = 1.00000;
const D65_Z: f64 = 1.08883;

pub fn srgb_to_linear(v: f64) -> f64 {
    if v <= 0.04045 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

pub fn linear_to_srgb(v: f64) -> f64 {
    if v <= 0.0031308 {
        v * 12.92
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

pub fn rgb_to_xyz(rgb: &Vector3<f64>) -> Vector3<f64> {
    let r = srgb_to_linear(rgb[0]);
    let g = srgb_to_linear(rgb[1]);
    let b = srgb_to_linear(rgb[2]);

    Vector3::new(
        r * 0.4124564 + g * 0.3575761 + b * 0.1804375,
        r * 0.2126729 + g * 0.7151522 + b * 0.0721750,
        r * 0.0193339 + g * 0.1191920 + b * 0.9503041,
    )
}

pub fn f_cbrt(t: f64) -> f64 {
    if t > 0.008856 {
        t.cbrt()
    } else {
        7.787 * t + 16.0 / 116.0
    }
}

pub fn f_cbrt_inv(t: f64) -> f64 {
    let t3 = t * t * t;
    if t3 > 0.008856 {
        t3
    } else {
        (t - 16.0 / 116.0) / 7.787
    }
}

pub fn xyz_to_lab(xyz: &Vector3<f64>) -> Vector3<f64> {
    let x = f_cbrt(xyz[0] / D65_X);
    let y = f_cbrt(xyz[1] / D65_Y);
    let z = f_cbrt(xyz[2] / D65_Z);

    Vector3::new(
        (116.0 * y) - 16.0,
        500.0 * (x - y),
        200.0 * (y - z),
    )
}

pub fn rgb_to_lab(rgb: &Vector3<f64>) -> Vector3<f64> {
    xyz_to_lab(&rgb_to_xyz(rgb))
}

pub fn lab_to_xyz(lab: &Vector3<f64>) -> Vector3<f64> {
    let l = lab[0];
    let a = lab[1];
    let b = lab[2];

    let y = (l + 16.0) / 116.0;
    let x = a / 500.0 + y;
    let z = y - b / 200.0;

    Vector3::new(
        D65_X * f_cbrt_inv(x),
        D65_Y * f_cbrt_inv(y),
        D65_Z * f_cbrt_inv(z),
    )
}

pub fn xyz_to_rgb(xyz: &Vector3<f64>) -> Vector3<f64> {
    let x = xyz[0];
    let y = xyz[1];
    let z = xyz[2];

    let r = x *  3.2404542 + y * -1.5371385 + z * -0.4985314;
    let g = x * -0.9692660 + y *  1.8760108 + z *  0.0415560;
    let b = x *  0.0556434 + y * -0.2040259 + z *  1.0572252;

    Vector3::new(
        linear_to_srgb(r).clamp(0.0, 1.0),
        linear_to_srgb(g).clamp(0.0, 1.0),
        linear_to_srgb(b).clamp(0.0, 1.0),
    )
}

pub fn lab_to_rgb(lab: &Vector3<f64>) -> Vector3<f64> {
    xyz_to_rgb(&lab_to_xyz(lab))
}

// ==========================================
// 2. K-Means++ Clustering
// ==========================================

pub fn kmeans_plus_plus(
    data: &[Vector3<f64>],
    k: usize,
    max_iters: usize,
) -> (Vec<Vector3<f64>>, Vec<usize>) {
    if data.is_empty() || k == 0 {
        return (vec![], vec![]);
    }
    
    let mut rng = StdRng::seed_from_u64(42);
    let n = data.len();
    let mut centroids = Vec::with_capacity(k);
    
    let first_idx = (rng.random::<f64>() * n as f64) as usize;
    let first_idx = first_idx.min(n - 1);
    centroids.push(data[first_idx].clone());
    
    for _ in 1..k {
        let mut dist_sq = vec![0.0; n];
        let mut sum_dist_sq = 0.0;
        
        for i in 0..n {
            let min_dist = centroids
                .iter()
                .map(|c: &Vector3<f64>| (data[i] - c).norm_squared())
                .fold(f64::INFINITY, f64::min);
            dist_sq[i] = min_dist;
            sum_dist_sq += min_dist;
        }
        
        let mut target = rng.random::<f64>() * sum_dist_sq;
        let mut next_idx = n - 1;
        for i in 0..n {
            target -= dist_sq[i];
            if target <= 0.0 {
                next_idx = i;
                break;
            }
        }
        centroids.push(data[next_idx].clone());
    }
    
    let mut assignments = vec![0; n];
    
    for _ in 0..max_iters {
        let mut changed = false;
        
        for i in 0..n {
            let mut best_k = 0;
            let mut min_dist = f64::INFINITY;
            
            for (j, c) in centroids.iter().enumerate() {
                let dist = (data[i] - c as &Vector3<f64>).norm_squared();
                if dist < min_dist {
                    min_dist = dist;
                    best_k = j;
                }
            }
            
            if assignments[i] != best_k {
                assignments[i] = best_k;
                changed = true;
            }
        }
        
        if !changed {
            break;
        }
        
        let mut new_centroids = vec![Vector3::zeros(); k];
        let mut counts = vec![0; k];
        
        for i in 0..n {
            let cluster = assignments[i];
            new_centroids[cluster] += data[i];
            counts[cluster] += 1;
        }
        
        for j in 0..k {
            if counts[j] > 0 {
                centroids[j] = new_centroids[j] / (counts[j] as f64);
            }
        }
    }
    
    (centroids, assignments)
}

// ==========================================
// 3. Sinkhorn Optimal Transport (OT)
// ==========================================

pub fn sinkhorn_ot(
    mu: &DVector<f64>,
    nu: &DVector<f64>,
    cost_matrix: &DMatrix<f64>,
    reg: f64,
    max_iters: usize,
    tolerance: f64,
) -> DMatrix<f64> {
    let n = mu.len();
    let m = nu.len();
    
    let mut k_mat = DMatrix::zeros(n, m);
    for i in 0..n {
        for j in 0..m {
            k_mat[(i, j)] = (-cost_matrix[(i, j)] / reg).exp();
        }
    }
    
    let mut u = DVector::from_element(n, 1.0 / n as f64);
    let mut v = DVector::from_element(m, 1.0 / m as f64);
    
    for _ in 0..max_iters {
        let u_prev = u.clone();
        
        let kt_u = k_mat.transpose() * &u;
        for j in 0..m {
            v[j] = nu[j] / kt_u[j].max(1e-15);
        }
        
        let k_v = &k_mat * &v;
        for i in 0..n {
            u[i] = mu[i] / k_v[i].max(1e-15);
        }
        
        let mut max_diff = 0.0_f64;
        for i in 0..n {
            max_diff = max_diff.max((u[i] - u_prev[i]).abs());
        }
        if max_diff < tolerance {
            break;
        }
    }
    
    let mut p = DMatrix::zeros(n, m);
    for i in 0..n {
        for j in 0..m {
            p[(i, j)] = u[i] * k_mat[(i, j)] * v[j];
        }
    }
    p
}

// ==========================================
// 4. Thin Plate Spline (TPS)
// ==========================================

pub struct TPS {
    src_points: Vec<Vector3<f64>>,
    weights: DMatrix<f64>,
}

impl TPS {
    pub fn fit(src_points: &[Vector3<f64>], dst_points: &[Vector3<f64>]) -> Option<Self> {
        let n = src_points.len();
        if n != dst_points.len() || n < 4 {
            return None; // 至少需要 4 个控制点
        }
        
        let mut k = DMatrix::zeros(n, n);
        for i in 0..n {
            for j in 0..n {
                let r = (src_points[i] - src_points[j]).norm();
                k[(i, j)] = Self::u(r);
            }
        }
        
        let mut p = DMatrix::zeros(n, 4);
        for i in 0..n {
            p[(i, 0)] = 1.0;
            p[(i, 1)] = src_points[i][0];
            p[(i, 2)] = src_points[i][1];
            p[(i, 3)] = src_points[i][2];
        }
        
        let mut l = DMatrix::zeros(n + 4, n + 4);
        for i in 0..n {
            for j in 0..n {
                l[(i, j)] = k[(i, j)];
            }
            for j in 0..4 {
                l[(i, n + j)] = p[(i, j)];
                l[(n + j, i)] = p[(i, j)];
            }
        }
        
        // 增加正则化项避免奇异矩阵
        for i in 0..(n + 4) {
            l[(i, i)] += 1e-4; // 增大正则化，避免点重合导致求解失败
        }
        
        let mut y = DMatrix::zeros(n + 4, 3);
        for i in 0..n {
            y[(i, 0)] = dst_points[i][0];
            y[(i, 1)] = dst_points[i][1];
            y[(i, 2)] = dst_points[i][2];
        }
        
        let decomp = l.lu();
        let weights = decomp.solve(&y)?;
        
        Some(Self {
            src_points: src_points.to_vec(),
            weights,
        })
    }
    
    pub fn transform(&self, point: &Vector3<f64>) -> Vector3<f64> {
        let n = self.src_points.len();
        let mut res = Vector3::zeros();
        
        // 仿射部分
        res[0] = self.weights[(n, 0)] + self.weights[(n+1, 0)] * point[0] + self.weights[(n+2, 0)] * point[1] + self.weights[(n+3, 0)] * point[2];
        res[1] = self.weights[(n, 1)] + self.weights[(n+1, 1)] * point[0] + self.weights[(n+2, 1)] * point[1] + self.weights[(n+3, 1)] * point[2];
        res[2] = self.weights[(n, 2)] + self.weights[(n+1, 2)] * point[0] + self.weights[(n+2, 2)] * point[1] + self.weights[(n+3, 2)] * point[2];
        
        // 非线性形变部分
        for i in 0..n {
            let r = (point - self.src_points[i]).norm();
            let u_r = Self::u(r);
            res[0] += self.weights[(i, 0)] * u_r;
            res[1] += self.weights[(i, 1)] * u_r;
            res[2] += self.weights[(i, 2)] * u_r;
        }
        
        res
    }
    
    // TPS 三维基函数
    fn u(r: f64) -> f64 {
        r
    }
}
