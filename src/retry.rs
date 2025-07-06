use std::time::{Duration, SystemTime};

pub struct Retry {
    max: usize,
    threshold: Duration,

    iterations: usize,
    over_threshold_iterations: usize,
    last_retry: Option<SystemTime>,
}

impl Retry {
    pub fn new(max: usize, threshold: Duration) -> Self {
        Self {
            max,
            threshold,
            last_retry: None,
            iterations: 0,
            over_threshold_iterations: 0,
        }
    }
}

impl Iterator for Retry {
    type Item = (usize, usize);

    fn next(&mut self) -> Option<Self::Item> {
        let now = SystemTime::now();

        if let Some(last_retry) = self.last_retry {
            let since_last_retry = now.duration_since(last_retry).ok()?;
            if since_last_retry < self.threshold {
                self.over_threshold_iterations += 1;
            } else {
                self.over_threshold_iterations -= 1;
            }
        }

        if self.over_threshold_iterations > self.max {
            return None;
        }

        self.last_retry = Some(SystemTime::now());
        self.iterations += 1;
        Some((
            self.iterations - 1,
            self.max - self.over_threshold_iterations,
        ))
    }
}

#[cfg(test)]
mod test {
    use crate::retry::Retry;
    use std::thread;
    use std::time::Duration;

    #[test]
    fn retry_basic() {
        let mut r = Retry::new(3, Duration::from_secs(3600));

        assert_eq!(r.next(), Some((0, 3))); // first one - taking first time measurement
        assert_eq!(r.next(), Some((1, 2))); // first over threashold
        assert_eq!(r.next(), Some((2, 1))); // second over threashold
        assert_eq!(r.next(), Some((3, 0))); // third over threashold
        assert_eq!(r.next(), None); // fourth over threashold -> exit
    }

    #[test]
    fn retry_with_reset() {
        let mut r = Retry::new(3, Duration::from_millis(10));

        assert_eq!(r.next(), Some((0, 3))); // first one - taking first time measurement
        assert_eq!(r.next(), Some((1, 2))); // first over threashold
        assert_eq!(r.next(), Some((2, 1))); // second over threashold

        thread::sleep(Duration::from_millis(100));
        assert_eq!(r.next(), Some((3, 2))); // Reset 1 from threshold

        assert_eq!(r.next(), Some((4, 1))); // second over threashold (again)
        assert_eq!(r.next(), Some((5, 0))); // third over threashold
        assert_eq!(r.next(), None); // fourth over threashold -> exit
    }
}
