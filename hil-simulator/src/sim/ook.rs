/// Deterministic pseudo-random noise in [-1, 1].
pub fn noise_sample(seed: u32) -> f32 {
    let mut x = seed.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
    x ^= x << 13;
    x ^= x >> 17;
    x ^= x << 5;
    (x as f32 / u32::MAX as f32) * 2.0 - 1.0
}

pub fn downsample(values: &[f32], target: usize) -> Vec<f32> {
    if values.is_empty() {
        return Vec::new();
    }
    if values.len() <= target {
        return values.to_vec();
    }
    let step = values.len() as f32 / target as f32;
    (0..target)
        .map(|i| {
            let idx = (i as f32 * step) as usize;
            values[idx.min(values.len() - 1)]
        })
        .collect()
}
