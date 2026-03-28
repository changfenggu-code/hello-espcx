use super::*;
use btleplus::ManufacturerData;

fn valid_manufacturer_data() -> ManufacturerData {
    ManufacturerData {
        company_id: advertisement_identity::DEVELOPMENT_COMPANY_ID,
        data: vec![
            advertisement_identity::VERSION,
            advertisement_identity::PRODUCT_ID_HELLO_ESPCX,
            0x78,
            0x56,
            0x34,
            0x12,
            0b0000_0010,
        ],
    }
}

#[test]
fn decode_manufacturer_payload_accepts_expected_layout() {
    let payload = decode_manufacturer_payload(&valid_manufacturer_data()).unwrap();

    assert_eq!(payload.version, advertisement_identity::VERSION);
    assert_eq!(
        payload.product_id,
        advertisement_identity::PRODUCT_ID_HELLO_ESPCX
    );
    assert_eq!(payload.unit_id, 0x1234_5678);
    assert_eq!(payload.flags, 0b0000_0010);
}

#[test]
fn decode_manufacturer_payload_rejects_wrong_company_or_length() {
    let wrong_company = ManufacturerData {
        company_id: advertisement_identity::DEVELOPMENT_COMPANY_ID.wrapping_add(1),
        data: valid_manufacturer_data().data,
    };
    assert!(decode_manufacturer_payload(&wrong_company).is_none());

    let wrong_length = ManufacturerData {
        company_id: advertisement_identity::DEVELOPMENT_COMPANY_ID,
        data: vec![advertisement_identity::VERSION],
    };
    assert!(decode_manufacturer_payload(&wrong_length).is_none());
}

#[test]
fn product_scan_filter_targets_expected_name_service_and_company() {
    let filter = build_product_scan_filter();

    assert_eq!(filter.name_patterns, vec![PERIPHERAL_NAME.to_string()]);
    assert_eq!(
        filter.service_uuids,
        vec![Uuid::from_u16(battery::SERVICE_UUID16)]
    );
    assert_eq!(
        filter.manufacturer_company_ids,
        vec![advertisement_identity::DEVELOPMENT_COMPANY_ID]
    );
}

#[test]
fn product_scan_filter_matches_expected_product_advertisement() {
    assert!(matches_product_identity(&valid_manufacturer_data()));
}

#[test]
fn product_scan_filter_rejects_wrong_product_payload() {
    let mut wrong_version = valid_manufacturer_data();
    wrong_version.data[0] = advertisement_identity::VERSION.wrapping_add(1);
    assert!(!matches_product_identity(&wrong_version));

    let mut wrong_product = valid_manufacturer_data();
    wrong_product.data[1] = advertisement_identity::PRODUCT_ID_HELLO_ESPCX.wrapping_add(1);
    assert!(!matches_product_identity(&wrong_product));
}
