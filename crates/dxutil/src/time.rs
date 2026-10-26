//! Monotonic clocks with a platform-dependent time origin.

/// Return monotonic milliseconds with an unspecified time origin.
/// Only differences between readings are meaningful.
///
/// On wasm32 with `web`, use `window.performance.now()`, or `0.0` when
/// unavailable. Other configurations use elapsed time since a process-wide
/// [`std::time::Instant`], initialized on the first call.
pub fn now_ms() -> f64 {
    #[cfg(all(feature = "web", target_arch = "wasm32"))]
    {
        web_sys::window()
            .and_then(|window| window.performance())
            .map_or(0.0, |performance| performance.now())
    }
    #[cfg(not(all(feature = "web", target_arch = "wasm32")))]
    {
        use std::{sync::OnceLock, time::Instant};
        static START: OnceLock<Instant> = OnceLock::new();
        START.get_or_init(Instant::now).elapsed().as_secs_f64() * 1000.0
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    #[test]
    fn clock_is_monotonic_and_advances() {
        let start = now_ms();
        let mut previous = start;
        for _ in 0..1000 {
            let now = now_ms();
            assert!(now >= previous);
            previous = now;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
        assert!(now_ms() > start);
    }
}
