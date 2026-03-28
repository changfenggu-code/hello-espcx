use super::*;
use std::collections::BTreeMap;

#[derive(Debug, Clone)]
struct FakeCandidate {
    id: String,
    properties: PeripheralProperties,
}

impl FakeCandidate {
    fn new(id: &str, local_name: Option<&str>, rssi: Option<i16>, connectable: bool) -> Self {
        Self {
            id: id.to_string(),
            properties: PeripheralProperties {
                id: id.to_string(),
                local_name: local_name.map(str::to_string),
                advertised_services: Vec::new(),
                manufacturer_data: None,
                service_data: BTreeMap::new(),
                rssi,
                is_connectable: connectable,
            },
        }
    }

    fn with_manufacturer_data(mut self, company_id: u16, data: &[u8]) -> Self {
        self.properties.manufacturer_data = Some(ManufacturerData {
            company_id,
            data: data.to_vec(),
        });
        self
    }
}

impl Candidate for FakeCandidate {
    fn id(&self) -> &str {
        &self.id
    }

    fn properties(&self) -> &PeripheralProperties {
        &self.properties
    }
}

#[test]
fn prefer_connectable_keeps_non_connectable_as_fallback() {
    let selector = Selector::default().prefer_connectable();
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-40), false),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-80), true),
    ];

    let chosen = selector.select_ref(&candidates).unwrap();
    assert_eq!(chosen.id(), "b");
}

#[test]
fn prefer_strongest_signal_picks_highest_rssi() {
    let selector = Selector::default().prefer_strongest_signal();
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-80), true),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-45), true),
        FakeCandidate::new("c", Some("hello-espcx"), Some(-60), true),
    ];

    let chosen = selector.select_ref(&candidates).unwrap();
    assert_eq!(chosen.id(), "b");
}

#[test]
fn filter_removes_non_matching_candidates() {
    let selector = Selector::default()
        .filter(|properties| properties.local_name.as_deref() == Some("hello-espcx"));
    let candidates = [
        FakeCandidate::new("a", Some("other"), Some(-40), true),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-80), true),
    ];

    let chosen = selector.select_ref(&candidates).unwrap();
    assert_eq!(chosen.id(), "b");
}

#[test]
fn no_candidates_after_filter_returns_selection_error() {
    let selector = Selector::default()
        .filter(|properties| properties.local_name.as_deref() == Some("missing"));
    let candidates = [FakeCandidate::new(
        "a",
        Some("hello-espcx"),
        Some(-40),
        true,
    )];

    let error = selector.select_ref(&candidates).unwrap_err();
    assert!(matches!(error, BtleplusError::SelectionFailed(_)));
}

#[test]
fn prefer_manufacturer_company_id_boosts_matching_candidate() {
    let selector = Selector::default().prefer_manufacturer_company_id(0x004C);
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-40), true)
            .with_manufacturer_data(0xFFFF, &[1, 2, 3]),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-80), true)
            .with_manufacturer_data(0x004C, &[9, 9, 9]),
    ];

    let chosen = selector.select_ref(&candidates).unwrap();
    assert_eq!(chosen.id(), "b");
}

#[test]
fn prefer_call_order_sets_priority() {
    let selector = Selector::default()
        .prefer_strongest_signal()
        .prefer_connectable();
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-80), true),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-45), true),
        FakeCandidate::new("c", Some("hello-espcx"), Some(-30), false),
    ];

    let ranked = selector.rank_ref(&candidates).unwrap();
    let ranked_ids: Vec<&str> = ranked.into_iter().map(|candidate| candidate.id()).collect();

    assert_eq!(ranked_ids, vec!["c", "b", "a"]);
}

#[test]
fn repeated_preference_calls_keep_their_chain_order() {
    let selector = Selector::default().prefer_id("a").prefer_id("b");
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-80), true),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-45), true),
        FakeCandidate::new("c", Some("hello-espcx"), Some(-30), true),
    ];

    let ranked = selector.rank_ref(&candidates).unwrap();
    let ranked_ids: Vec<&str> = ranked.into_iter().map(|candidate| candidate.id()).collect();

    assert_eq!(ranked_ids, vec!["a", "b", "c"]);
}

#[test]
fn rank_returns_all_candidates_in_preference_order() {
    let selector = Selector::default()
        .prefer_connectable()
        .prefer_strongest_signal();
    let candidates = [
        FakeCandidate::new("a", Some("hello-espcx"), Some(-80), true),
        FakeCandidate::new("b", Some("hello-espcx"), Some(-45), true),
        FakeCandidate::new("c", Some("hello-espcx"), Some(-30), false),
    ];

    let ranked = selector.rank_ref(&candidates).unwrap();
    let ranked_ids: Vec<&str> = ranked.into_iter().map(|candidate| candidate.id()).collect();

    assert_eq!(ranked_ids, vec!["b", "a", "c"]);
}
