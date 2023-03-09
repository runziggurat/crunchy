use std::{collections::HashMap, io::Write, net::SocketAddr};

use crate::ips::algorithm::IpsState;

/// This struct is used to store statistics for network at some point in time.
pub struct Statistics {
    nodes_count: usize,
    degree_average: f64,
    degree_median: f64,
    degree_min: f64,
    degree_max: f64,
    betweenness_average: f64,
    betweenness_median: f64,
    betweenness_min: f64,
    betweenness_max: f64,
    closeness_average: f64,
    closeness_median: f64,
    closeness_min: f64,
    closeness_max: f64,
    eigenvector_average: f64,
    eigenvector_median: f64,
    eigenvector_min: f64,
    eigenvector_max: f64,
}

/// Calculates statistics for given network state.
pub fn generate_statistics(state: &IpsState) -> Statistics {
    Statistics {
        nodes_count: state.nodes.len(),

        degree_average: degree_centrality_avg(&state.degrees),
        degree_median: median::<u32>(&state.degrees.values().copied().collect::<Vec<u32>>())
            .expect("can't calculate median"),
        degree_min: state.degree_factors.min,
        degree_max: state.degree_factors.max,

        betweenness_average: centrality_avg(
            &state
                .nodes
                .iter()
                .map(|n| n.betweenness)
                .collect::<Vec<f64>>(),
        ),
        betweenness_median: median::<f64>(
            &state
                .nodes
                .iter()
                .map(|n| n.betweenness)
                .collect::<Vec<f64>>(),
        )
        .expect("can't calculate median"),
        betweenness_min: state.betweenness_factors.min,
        betweenness_max: state.betweenness_factors.max,

        closeness_average: centrality_avg(
            &state
                .nodes
                .iter()
                .map(|n| n.closeness)
                .collect::<Vec<f64>>(),
        ),
        closeness_median: median::<f64>(
            &state
                .nodes
                .iter()
                .map(|n| n.closeness)
                .collect::<Vec<f64>>(),
        )
        .expect("can't calculate median"),
        closeness_min: state.closeness_factors.min,
        closeness_max: state.closeness_factors.max,

        eigenvector_average: centrality_avg(
            &state.eigenvalues.values().copied().collect::<Vec<f64>>(),
        ),
        eigenvector_median: median::<f64>(
            &state.eigenvalues.values().copied().collect::<Vec<f64>>(),
        )
        .expect("can't calculate median"),
        eigenvector_min: state.eigenvector_factors.min,
        eigenvector_max: state.eigenvector_factors.max,
    }
}

/// Prints statistics to given output.
pub fn print_statistics(output: &mut Box<dyn Write>, stats: &Statistics) {
    writeln!(output, "----------------------------------------").unwrap();
    writeln!(output, "Nodes count: {}", stats.nodes_count).unwrap();
    writeln!(output, "\nDegree measures:").unwrap();
    writeln!(output, "Average: {}", stats.degree_average).unwrap();
    writeln!(output, "Median: {}", stats.degree_median).unwrap();
    writeln!(
        output,
        "Min: {}, max: {}, delta: {}",
        stats.degree_min,
        stats.degree_max,
        stats.degree_max - stats.degree_min
    )
    .unwrap();

    writeln!(output, "\nBetweenness measures:").unwrap();
    writeln!(output, "Average: {}", stats.betweenness_average).unwrap();
    writeln!(output, "Median: {}", stats.betweenness_median).unwrap();
    writeln!(
        output,
        "Min: {}, max: {}, delta: {}",
        stats.betweenness_min,
        stats.betweenness_max,
        stats.betweenness_max - stats.betweenness_min
    )
    .unwrap();

    writeln!(output, "\nCloseness measures:").unwrap();
    writeln!(output, "Average: {}", stats.closeness_average).unwrap();
    writeln!(output, "Median: {}", stats.closeness_median).unwrap();
    writeln!(
        output,
        "Min: {}, max: {}, delta: {}",
        stats.closeness_min,
        stats.closeness_max,
        stats.closeness_max - stats.closeness_min
    )
    .unwrap();

    writeln!(output, "\nEigenvector measures:").unwrap();
    writeln!(output, "Average: {}", stats.eigenvector_average).unwrap();
    writeln!(output, "Median: {}", stats.eigenvector_median).unwrap();
    writeln!(
        output,
        "Min: {}, max: {}, delta: {}",
        stats.eigenvector_min,
        stats.eigenvector_max,
        stats.eigenvector_max - stats.eigenvector_min
    )
    .unwrap();

    writeln!(output, "----------------------------------------\n").unwrap();
}

/// Calculates percentage change between two values.
fn percentage_change(original: f64, new: f64) -> f64 {
    // Calc delta to keep the original value intact for this part
    let delta = new - original;

    let mut original = original;
    if original == 0.0 {
        // We can use some small value here just to fake the infinity case (diving by zero)
        original = 0.000000001;
    }

    (delta / original) * 100.0
}

/// Print statistics delta (value and percentage) between two statistics.
pub fn print_statistics_delta(
    output: &mut Box<dyn Write>,
    stats: &Statistics,
    stats_original: &Statistics,
) {
    writeln!(output, "Deltas for given statistics pair:").unwrap();
    writeln!(output, "----------------------------------------").unwrap();
    writeln!(
        output,
        "Nodes count: {} ({:.3}%)",
        stats.nodes_count - stats_original.nodes_count,
        percentage_change(stats_original.nodes_count as f64, stats.nodes_count as f64)
    )
    .unwrap();
    writeln!(output, "\nDegree measures:").unwrap();
    writeln!(
        output,
        "Average: {} ({:.3}%)",
        stats.degree_average - stats_original.degree_average,
        percentage_change(stats_original.degree_average, stats.degree_average)
    )
    .unwrap();
    writeln!(
        output,
        "Median: {} ({:.3}%)",
        stats.degree_median - stats_original.degree_median,
        percentage_change(stats_original.degree_median, stats.degree_median)
    )
    .unwrap();
    writeln!(
        output,
        "Min: {} ({:.3}%), max: {} ({:.3}%), delta: {} ({:.3}%)",
        stats.degree_min - stats_original.degree_min,
        percentage_change(stats_original.degree_min, stats.degree_min),
        stats.degree_max - stats_original.degree_max,
        percentage_change(stats_original.degree_max, stats.degree_max),
        stats.degree_max
            - stats.degree_min
            - (stats_original.degree_max - stats_original.degree_min),
        percentage_change(
            stats_original.degree_max - stats_original.degree_min,
            stats.degree_max - stats.degree_min
        )
    )
    .unwrap();

    writeln!(output, "\nBetweenness measures:").unwrap();
    writeln!(
        output,
        "Average: {} ({:.3}%)",
        stats.betweenness_average - stats_original.betweenness_average,
        percentage_change(
            stats_original.betweenness_average,
            stats.betweenness_average
        )
    )
    .unwrap();
    writeln!(
        output,
        "Median: {} ({:.3}%)",
        stats.betweenness_median - stats_original.betweenness_median,
        percentage_change(stats_original.betweenness_median, stats.betweenness_median)
    )
    .unwrap();
    writeln!(
        output,
        "Min: {} ({:.3}%), max: {} ({:.3}%), delta: {} ({:.3}%)",
        stats.betweenness_min - stats_original.betweenness_min,
        percentage_change(stats_original.betweenness_min, stats.betweenness_min),
        stats.betweenness_max - stats_original.betweenness_max,
        percentage_change(stats_original.betweenness_max, stats.betweenness_max),
        stats.betweenness_max
            - stats.betweenness_min
            - (stats_original.betweenness_max - stats_original.betweenness_min),
        percentage_change(
            stats_original.betweenness_max - stats_original.betweenness_min,
            stats.betweenness_max - stats.betweenness_min
        )
    )
    .unwrap();

    writeln!(output, "\nCloseness measures:").unwrap();
    writeln!(
        output,
        "Average: {} ({:.3}%)",
        stats.closeness_average - stats_original.closeness_average,
        percentage_change(stats_original.closeness_average, stats.closeness_average)
    )
    .unwrap();
    writeln!(
        output,
        "Median: {} ({:.3}%)",
        stats.closeness_median - stats_original.closeness_median,
        percentage_change(stats_original.closeness_median, stats.closeness_median)
    )
    .unwrap();
    writeln!(
        output,
        "Min: {} ({:.3}%), max: {} ({:.3}%), delta: {} ({:.3}%)",
        stats.closeness_min - stats_original.closeness_min,
        percentage_change(stats_original.closeness_min, stats.closeness_min),
        stats.closeness_max - stats_original.closeness_max,
        percentage_change(stats_original.closeness_max, stats.closeness_max),
        stats.closeness_max
            - stats.closeness_min
            - (stats_original.closeness_max - stats_original.closeness_min),
        percentage_change(
            stats_original.closeness_max - stats_original.closeness_min,
            stats.closeness_max - stats.closeness_min
        )
    )
    .unwrap();

    writeln!(output, "\nEigenvector measures:").unwrap();
    writeln!(
        output,
        "Average: {} ({:.3}%)",
        stats.eigenvector_average - stats_original.eigenvector_average,
        percentage_change(
            stats_original.eigenvector_average,
            stats.eigenvector_average
        )
    )
    .unwrap();
    writeln!(
        output,
        "Median: {} ({:.3}%)",
        stats.eigenvector_median - stats_original.eigenvector_median,
        percentage_change(stats_original.eigenvector_median, stats.eigenvector_median)
    )
    .unwrap();
    writeln!(
        output,
        "Min: {} ({:.3}%), max: {} ({:.3}%), delta: {} ({:.3}%)",
        stats.eigenvector_min - stats_original.eigenvector_min,
        percentage_change(stats_original.eigenvector_min, stats.eigenvector_min),
        stats.eigenvector_max - stats_original.eigenvector_max,
        percentage_change(stats_original.eigenvector_max, stats.eigenvector_max),
        stats.eigenvector_max
            - stats.eigenvector_min
            - (stats_original.eigenvector_max - stats_original.eigenvector_min),
        percentage_change(
            stats_original.eigenvector_max - stats_original.eigenvector_min,
            stats.eigenvector_max - stats.eigenvector_min
        )
    )
    .unwrap();

    writeln!(output, "----------------------------------------\n").unwrap();
}

/// Measures the average degree of the graph.
pub fn degree_centrality_avg(degrees: &HashMap<SocketAddr, u32>) -> f64 {
    if degrees.is_empty() {
        return 0.0;
    }

    (degrees.iter().fold(0, |acc, (_, &degree)| acc + degree) as f64) / degrees.len() as f64
}

/// Measures the average of any float value.
pub fn centrality_avg(values: &[f64]) -> f64 {
    if values.is_empty() {
        return 0.0;
    }

    (values.iter().fold(0.0, |acc, &val| acc + val)) / values.len() as f64
}

/// Computes median of any numeric type convertible to float value.
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
    use std::{
        net::{IpAddr, SocketAddr},
        str::FromStr,
    };

    use super::*;

    #[test]
    fn percentage_change_test() {
        assert!(percentage_change(100.0, 200.0) - 100.0 < 0.0001);
        assert!(percentage_change(100.0, 50.0) - -50.0 < 0.0001);
        assert!(percentage_change(100.0, 100.0) - 0.0 < 0.0001);
        assert!(percentage_change(0.0, 0.0) - 0.0 < 0.0001);
    }

    #[test]
    fn degree_centrality_avg_test() {
        let mut degrees = HashMap::new();
        degrees.insert(
            SocketAddr::new(IpAddr::from_str("0.0.0.0").unwrap(), 1234),
            1,
        );
        degrees.insert(
            SocketAddr::new(IpAddr::from_str("1.0.0.0").unwrap(), 1234),
            2,
        );
        degrees.insert(
            SocketAddr::new(IpAddr::from_str("2.0.0.0").unwrap(), 1234),
            3,
        );
        degrees.insert(
            SocketAddr::new(IpAddr::from_str("3.0.0.0").unwrap(), 1234),
            4,
        );

        assert!(degree_centrality_avg(&degrees) - 2.5 < 0.0001);
    }

    #[test]
    fn degree_centrality_avg_empty_test() {
        let degrees = HashMap::new();

        assert_eq!(degree_centrality_avg(&degrees), 0.0);
    }

    #[test]
    fn centrality_avg_empty_test() {
        let vals: Vec<f64> = Vec::new();

        assert_eq!(centrality_avg(&vals), 0.0);
    }

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
