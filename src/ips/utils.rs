/// Calculates the median of a list of numbers.
pub fn median<T>(list: &[T]) -> Option<f64>
where
    T: PartialOrd + Into<f64> + Copy,
{
    if list.is_empty() {
        return None;
    }

    let mut list = list.to_vec();
    list.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = list.len() / 2;
    if list.len() % 2 == 0 {
        Some((list[mid - 1].into() + list[mid].into()) / 2.0)
    } else {
        Some(list[mid].into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn median_test() {
        let list = vec![10];
        assert_eq!(median(&list).unwrap(), 10.0);

        let list = vec![1, 2, 3, 4, 5];
        assert_eq!(median(&list).unwrap(), 3.0);

        let list = vec![1, 2, 3, 4, 5, 6];
        assert_eq!(median(&list).unwrap(), 3.5);

        let list = vec![1, 2, 3, 4, 5, 6, 7];
        assert_eq!(median(&list).unwrap(), 4.0);
    }

    #[test]
    fn median_test_empty() {
        let list = Vec::<f64>::new();
        assert!(median(&list).is_none());
    }
}
