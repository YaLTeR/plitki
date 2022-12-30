use plitki_core::state::{EventKind, Hit};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Statistics {
    hits: Vec<u64>,
}

impl Statistics {
    pub fn new() -> Self {
        Self { hits: vec![0; 6] }
    }

    pub fn process_event(&mut self, kind: EventKind) {
        let index = match kind {
            EventKind::Hit(Hit { difference, .. }) => {
                // Quaver Standard judgements.
                match difference.into_milli_hundredths().abs() / 100 {
                    0..=18 => 0,
                    19..=43 => 1,
                    44..=76 => 2,
                    77..=106 => 3,
                    107..=127 => 4,
                    _ => 5,
                }
            }
            EventKind::Miss => 5,
        };

        self.hits[index] += 1;
    }

    pub fn accuracy(&self) -> f32 {
        let count: u64 = self.hits.iter().sum();

        if count == 0 {
            return 100.;
        }

        // Quaver weights.
        const WEIGHTS: [f32; 6] = [100., 98.25, 65., 25., -100., -50.];

        let accuracy: f32 = self
            .hits
            .iter()
            .zip(WEIGHTS)
            .map(|(&count, weight)| count as f32 * weight)
            .sum();

        let norm = count as f32 * WEIGHTS[0];
        (accuracy / norm).max(0.) * 100.
    }
}

impl Default for Statistics {
    fn default() -> Self {
        Self::new()
    }
}
