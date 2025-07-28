use std::time::Duration;

const NANOS_PER_SEC: u128 = 1_000_000_000;

pub trait DurationExt: Sized {
    fn as_duration(self) -> Duration;

    // Infuriating Orphan rule doesn't allow me to `impl Rem<Duration> for Duration`.
    // *Why* doesn't Rust have features as simple as this?
    fn rem(self, rhs: Duration) -> Duration {
        assert!(rhs != Duration::ZERO, "attempted to `%` a Duration by zero");

        let lhs = self.as_duration();
        if lhs < rhs {
            return lhs;
        }

        let lhs_nanos = (lhs.as_secs() as u128) * NANOS_PER_SEC + u128::from(lhs.subsec_nanos());
        let rhs_nanos = (rhs.as_secs() as u128) * NANOS_PER_SEC + u128::from(rhs.subsec_nanos());
        let rem_nanos = lhs_nanos % rhs_nanos;

        let secs = (rem_nanos / NANOS_PER_SEC) as u64;
        let nanos = (rem_nanos % NANOS_PER_SEC) as u32;
        Duration::new(secs, nanos)
    }
}

impl DurationExt for Duration {
    fn as_duration(self) -> Duration {
        self
    }
}
