use super::*;
use crate::gap::ManufacturerData;
use std::collections::BTreeMap;

fn props(id: &str, local_name: Option<&str>) -> PeripheralProperties {
    PeripheralProperties {
        id: id.to_string(),
        local_name: local_name.map(str::to_string),
        advertised_services: Vec::new(),
        manufacturer_data: None,
        service_data: BTreeMap::new(),
        rssi: Some(-50),
        is_connectable: true,
    }
}

#[test]
fn discover_helper_collects_first_matching_id() {
    let filter = ScanFilter::default().with_name_pattern("hello");
    let mut seen_ids = HashSet::new();
    let properties = props("id-a", Some("hello-espcx"));

    assert!(should_collect_discovered_properties(
        &properties,
        &filter,
        &mut seen_ids
    ));
    assert!(seen_ids.contains("id-a"));
}

#[test]
fn discover_helper_skips_non_matching_properties() {
    let filter = ScanFilter::default().with_name_pattern("hello");
    let mut seen_ids = HashSet::new();
    let properties = props("id-a", Some("other"));

    assert!(!should_collect_discovered_properties(
        &properties,
        &filter,
        &mut seen_ids
    ));
    assert!(seen_ids.is_empty());
}

#[test]
fn discover_helper_deduplicates_by_device_id() {
    let filter = ScanFilter::default().with_name_pattern("hello");
    let mut seen_ids = HashSet::new();
    let first = props("id-a", Some("hello-espcx"));
    let second = props("id-a", Some("hello-espcx"));

    assert!(should_collect_discovered_properties(
        &first,
        &filter,
        &mut seen_ids
    ));
    assert!(!should_collect_discovered_properties(
        &second,
        &filter,
        &mut seen_ids
    ));
}

#[test]
fn discover_helper_applies_manufacturer_filters_before_collecting() {
    let filter = ScanFilter::default().with_manufacturer_company_id(0x004C);
    let mut seen_ids = HashSet::new();
    let mut properties = props("id-a", Some("hello-espcx"));
    properties.manufacturer_data = Some(ManufacturerData {
        company_id: 0x004C,
        data: vec![1, 2, 3],
    });

    assert!(should_collect_discovered_properties(
        &properties,
        &filter,
        &mut seen_ids
    ));

    let mut wrong_company = props("id-b", Some("hello-espcx"));
    wrong_company.manufacturer_data = Some(ManufacturerData {
        company_id: 0xFFFF,
        data: vec![1, 2, 3],
    });

    assert!(!should_collect_discovered_properties(
        &wrong_company,
        &filter,
        &mut seen_ids
    ));
}
