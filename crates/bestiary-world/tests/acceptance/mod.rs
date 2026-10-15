fn partition(value: Option<&str>) -> Vec<u64> {
    match value {
        Some(value) => {
            let seed: u64 = value.parse().expect("acceptance seed must be an integer");
            assert!(seed < 5, "acceptance seed must be in 0..5");
            vec![seed]
        }
        None => (0..5).collect(),
    }
}
pub fn seeds() -> Vec<u64> {
    partition(std::env::var("BESTIARY_ACCEPTANCE_SEED").ok().as_deref())
}
#[test]
fn five_seed_partitions_cover_each_required_seed_exactly_once() {
    assert_eq!(partition(None), vec![0, 1, 2, 3, 4]);
    assert_eq!(
        (0..5)
            .flat_map(|s| partition(Some(&s.to_string())))
            .collect::<Vec<_>>(),
        partition(None)
    );
}
#[test]
#[should_panic(expected = "acceptance seed must be in 0..5")]
fn acceptance_seed_partition_rejects_out_of_range() {
    partition(Some("5"));
}
