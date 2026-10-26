//! SplitMix64 with an explicit seed; deterministic on every target.
pub(super) struct Rng(u64);
impl Rng {
    pub(super) fn new(seed: u64) -> Self {
        Self(seed)
    }
    pub(super) fn next(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9e3779b97f4a7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d049bb133111eb);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn stable_and_bounded() {
        let mut a = Rng::new(1);
        let mut b = Rng::new(1);
        for _ in 0..100 {
            let v = a.next();
            assert_eq!(v, b.next());
            assert!((0.0..1.0).contains(&v));
        }
    }
}
