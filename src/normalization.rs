/// Structure used to determine min and max values for normalization of any factor (like
/// betweenness centrality or closeness centrality).
#[derive(Default, Clone, Copy)]
pub struct NormalizationFactors {
    /// Minimum value of a factor.
    pub min: f64,
    /// Maximum value of a factor.
    pub max: f64,
}

impl NormalizationFactors {
    /// Determine min and max values for normalization.
    pub fn determine<T>(list: &[T]) -> NormalizationFactors
    where
        T: PartialOrd + Into<f64> + Copy,
    {
        let min = list
            .iter()
            .min_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();
        let max = list
            .iter()
            .max_by(|a, b| a.partial_cmp(b).unwrap())
            .unwrap();

        NormalizationFactors {
            min: (*min).into(),
            max: (*max).into(),
        }
    }

    /// Scale value to [0.0, 1.0] range.
    pub fn scale(&self, value: f64) -> f64 {
        if self.min == self.max {
            return 0.0;
        }

        (value - self.min) / (self.max - self.min)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalization_factors_determine_test() {
        let list = vec![1, 2, 3, 4, 5];
        let factors = NormalizationFactors::determine(&list);

        assert_eq!(factors.min, 1.0);
        assert_eq!(factors.max, 5.0);
    }

    #[test]
    fn normalization_factors_scale_test() {
        let factors = NormalizationFactors { min: 1.0, max: 5.0 };
        let value = 3.0;

        assert_eq!(factors.scale(value), 0.5);
    }

    #[test]
    fn normalization_factors_scale_divide_zero_test() {
        let factors = NormalizationFactors { min: 2.0, max: 2.0 };
        let value = 3.0;

        assert_eq!(factors.scale(value), 0.0);
    }
}
