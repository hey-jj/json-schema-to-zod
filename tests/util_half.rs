use json_schema_to_zod::half;

#[test]
fn splits_odd_length_with_longer_right() {
    let (a, b) = half(&["A", "B", "C", "D", "E"]);
    assert_eq!(a, vec!["A", "B"]);
    assert_eq!(b, vec!["C", "D", "E"]);
}
