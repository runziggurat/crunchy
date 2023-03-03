use std::{collections::HashMap, net::SocketAddr};

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

pub fn print_statistics(stats: &Statistics) {
    println!("----------------------------------------\n");
    println!("Nodes count: {}", stats.nodes_count);
    println!("\nDegree measures:");
    println!("Average: {}", stats.degree_average);
    println!("Median: {}", stats.degree_median);
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.degree_min,
        stats.degree_max,
        stats.degree_max - stats.degree_min
    );

    println!("\nBetweenness measures:");
    println!("Average: {}", stats.betweenness_average);
    println!("Median: {}", stats.betweenness_median);
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.betweenness_min,
        stats.betweenness_max,
        stats.betweenness_max - stats.betweenness_min
    );

    println!("\nCloseness measures:");
    println!("Average: {}", stats.closeness_average);
    println!("Median: {}", stats.closeness_median);
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.closeness_min,
        stats.closeness_max,
        stats.closeness_max - stats.closeness_min
    );

    println!("\nEigenvector measures:");
    println!("Average: {}", stats.eigenvector_average);
    println!("Median: {}", stats.eigenvector_median);
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.eigenvector_min,
        stats.eigenvector_max,
        stats.eigenvector_max - stats.eigenvector_min
    );

    println!("----------------------------------------\n");
}

pub fn print_statistics_delta(stats: &Statistics, stats_original: &Statistics) {
    println!("Deltas for given statistics pair:");
    println!("----------------------------------------\n");
    println!(
        "Nodes count: {}",
        stats.nodes_count - stats_original.nodes_count
    );
    println!("\nDegree measures:");
    println!(
        "Average: {}",
        stats.degree_average - stats_original.degree_average
    );
    println!(
        "Median: {}",
        stats.degree_median - stats_original.degree_median
    );
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.degree_min - stats_original.degree_min,
        stats.degree_max - stats_original.degree_max,
        stats.degree_max
            - stats.degree_min
            - (stats_original.degree_max - stats_original.degree_min)
    );

    println!("\nBetweenness measures:");
    println!(
        "Average: {}",
        stats.betweenness_average - stats_original.betweenness_average
    );
    println!(
        "Median: {}",
        stats.betweenness_median - stats_original.betweenness_median
    );
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.betweenness_min - stats_original.betweenness_min,
        stats.betweenness_max - stats_original.betweenness_max,
        stats.betweenness_max
            - stats.betweenness_min
            - (stats_original.betweenness_max - stats_original.betweenness_min)
    );

    println!("\nCloseness measures:");
    println!(
        "Average: {}",
        stats.closeness_average - stats_original.closeness_average
    );
    println!(
        "Median: {}",
        stats.closeness_median - stats_original.closeness_median
    );
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.closeness_min - stats_original.closeness_min,
        stats.closeness_max - stats_original.closeness_max,
        stats.closeness_max
            - stats.closeness_min
            - (stats_original.closeness_max - stats_original.closeness_min)
    );

    println!("\nEigenvector measures:");
    println!(
        "Average: {}",
        stats.eigenvector_average - stats_original.eigenvector_average
    );
    println!(
        "Median: {}",
        stats.eigenvector_median - stats_original.eigenvector_median
    );
    println!(
        "Min: {}, max: {}, delta: {}",
        stats.eigenvector_min - stats_original.eigenvector_min,
        stats.eigenvector_max - stats_original.eigenvector_max,
        stats.eigenvector_max
            - stats.eigenvector_min
            - (stats_original.eigenvector_max - stats_original.eigenvector_min)
    );

    println!("----------------------------------------\n");
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
