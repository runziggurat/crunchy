/// Calculates the median of a list of numbers.
pub fn median<T>(list: &[T]) -> f64
where
    T: PartialOrd + Into<f64> + Copy,
{
    let mut list = list.to_vec();
    list.sort_by(|a, b| a.partial_cmp(b).unwrap());

    let mid = list.len() / 2;
    if list.len() % 2 == 0 {
        (list[mid - 1].into() + list[mid].into()) / 2.0
    } else {
        list[mid].into()
    }
}
