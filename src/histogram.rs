/// Structure used to create a distribution of node
/// centrality values for use in a displayed histogram

#[derive(Default, Clone)]
pub struct Histogram {
    /// Minimum value of a factor.
    pub min: f64,
    /// Maximum value of a factor.
    pub max: f64,
    /// store values
    pub values: Vec<f64>,
}

impl Histogram {
    pub fn default() -> Self {
        Self {
            min: f64::MAX,
            max: f64::MIN,
            values: Vec::new(),
        }
    }

    pub fn add(&mut self, value: f64) {
        self.values.push(value);
    }

    pub fn compute(&mut self, num_counts: usize) -> (Vec<usize>, usize) {
        for value in self.values.iter() {
            if *value < self.min {
                self.min = *value;
            }
            if *value > self.max {
                self.max = *value;
            }
        }

        let delta = self.max - self.min;
        let mut counts = vec![0; num_counts];
        let mut max_count: usize = 0;

        if delta == 0.0 {
            return (counts, max_count);
        }

        for value in self.values.iter() {
            let part = (*value - self.min) / delta;
            let mut slot = (part * num_counts as f64).floor() as usize;
            if slot >= num_counts {
                slot = num_counts - 1;
            }
            counts[slot] += 1;
        }

        for count in counts.iter() {
            if *count > max_count {
                max_count = *count;
            }
        }

        (counts, max_count)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn histogram_test() {
        let mut histogram = Histogram {
            ..Histogram::default()
        };

        histogram.add(0.1);
        histogram.add(0.6);
        histogram.add(0.7);
        histogram.add(17.2);
        histogram.add(117.3);
        histogram.add(117.4);
        histogram.add(117.5);
        histogram.add(255.5);
        histogram.add(255.6);
        histogram.add(255.7);
        histogram.add(255.8);
        let (counts, max_count) = histogram.compute(256);
        assert_eq!(counts.len(), 256);
        assert_eq!(max_count, 4);
        assert_eq!(counts[16], 0);
        assert_eq!(counts[17], 1);
        assert_eq!(counts[18], 0);
        assert_eq!(counts[116], 0);
        assert_eq!(counts[117], 3);
    }

    #[test]
    fn histogram_zero_delta_test() {
        let mut histogram = Histogram {
            ..Histogram::default()
        };
        let (counts, max_count) = histogram.compute(256);
        assert_eq!(counts.len(), 256);
        assert_eq!(max_count, 0);

        histogram.add(4.6);
        histogram.add(4.6);
        histogram.add(4.6);
        histogram.add(4.6);
        let (counts, max_count) = histogram.compute(256);
        assert_eq!(counts.len(), 256);
        assert_eq!(max_count, 0);
        assert_eq!(counts[16], 0);
        assert_eq!(counts[116], 0);
        assert_eq!(counts[216], 0);
    }
}
