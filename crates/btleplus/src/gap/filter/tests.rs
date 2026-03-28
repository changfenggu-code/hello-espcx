use super::*;
use std::collections::BTreeMap;

/// Helper: create minimal PeripheralProperties for testing.
/// 辅助函数：创建最小化的 PeripheralProperties 用于测试。
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
fn matches_name_when_only_name_filters_exist() {
    let filter = ScanFilter::default().with_name_pattern("hello");
    assert!(filter.matches_properties(&props("id-a", Some("hello-espcx"))));
    assert!(!filter.matches_properties(&props("id-b", Some("other"))));
}

#[test]
fn matches_addr_when_only_addr_filters_exist() {
    let filter = ScanFilter::default().with_addr_pattern("device-123");
    assert!(filter.matches_properties(&props("device-123-abc", Some("other"))));
    assert!(!filter.matches_properties(&props("device-999", Some("other"))));
}

#[test]
fn manufacturer_company_id_filters_candidates() {
    let filter = ScanFilter::default().with_manufacturer_company_id(0xFFFF);
    let mut peripheral = props("id-a", Some("hello"));
    peripheral.manufacturer_data = Some(ManufacturerData {
        company_id: 0xFFFF,
        data: vec![1, 2, 3],
    });
    assert!(filter.matches_properties(&peripheral));

    peripheral.manufacturer_data = Some(ManufacturerData {
        company_id: 0x004C,
        data: vec![1, 2, 3],
    });
    assert!(!filter.matches_properties(&peripheral));
}
