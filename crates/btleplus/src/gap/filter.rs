//! ScanFilter — GAP scanning filter.
//! ScanFilter — GAP 扫描过滤器。
//!
//! # Public API / 公开 API
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`ScanFilter::with_name_pattern`] | Filter by single name pattern. 添加单个名称过滤模式。 |
//! | [`ScanFilter::with_name_patterns`] | Filter by multiple name patterns. 添加多个名称过滤模式。 |
//! | [`ScanFilter::with_addr_pattern`] | Filter by single address pattern. 添加单个地址过滤模式。 |
//! | [`ScanFilter::with_addr_patterns`] | Filter by multiple address patterns. 添加多个地址过滤模式。 |
//! | [`ScanFilter::with_service_uuid`] | Filter by single service UUID (OS-level). 添加单个服务 UUID 过滤（OS 级别）。 |
//! | [`ScanFilter::with_service_uuids`] | Filter by multiple service UUIDs. 添加多个服务 UUID 过滤。 |
//! | [`ScanFilter::with_manufacturer_company_id`] | Filter by manufacturer company ID. 按厂商公司 ID 过滤。 |
//! | [`ScanFilter::with_manufacturer_company_ids`] | Filter by multiple manufacturer company IDs. 按多个厂商公司 ID 过滤。 |
//! | [`ScanFilter::with_manufacturer_data`] | Filter by manufacturer data predicate. 按厂商数据断言过滤。 |
//! | [`ScanFilter::filter`] | Filter by arbitrary properties predicate. 按任意属性断言过滤。 |
//! | [`ScanFilter::with_scan_interval_secs`] | Set scan interval in seconds. 设置扫描间隔（秒）。 |
//!
//! # Filtering logic / 过滤逻辑
//!
//! Filters can be combined. Within each category the logic is OR:
//! 可以组合过滤器。每个类别内的所有条件是 OR 关系：
//! - Empty `name_patterns` / `addr_patterns` matches all.
//!   空的 `name_patterns` / `addr_patterns` 匹配所有。
//! - Non-empty filters match if device matches **any** pattern (prefix supported).
//!   非空过滤器在设备匹配**任意**模式时通过（支持前缀匹配）。
//!
//! Between categories the logic is AND: all categories must pass.
//! 类别之间是 AND 关系：所有类别都必须通过。

use bluest::Uuid;
use std::sync::Arc;

use super::peripheral::{ManufacturerData, ManufacturerPredicate, PeripheralProperties, PropertiesPredicate};

/// Scan filter for discovering peripherals.
/// 用于发现外设的扫描过滤器。
///
/// This is the **first-layer** filter applied during scanning (before `Selector`).
/// Use it to narrow down which devices are collected at scan time.
/// 这是扫描期间应用的**第一层**过滤器（在 `Selector` 之前）。
/// 用于在扫描时缩小收集范围。
#[derive(Clone, Default)]
pub struct ScanFilter {
    /// Filter by peripheral name patterns (OR'd, prefix matching supported).
    /// 按外设名称模式过滤（OR 关系，支持前缀匹配）。
    pub name_patterns: Vec<String>,
    /// Filter by address/device-id patterns (OR'd, prefix matching supported).
    /// 按地址/设备 ID 模式过滤（OR 关系，支持前缀匹配）。
    pub addr_patterns: Vec<String>,
    /// Filter by service UUIDs (OS-level scan filter).
    /// 按服务 UUID 过滤（操作系统级别扫描过滤器）。
    pub service_uuids: Vec<Uuid>,
    /// Hard filter by manufacturer company identifiers.
    /// 按厂商公司标识符硬性过滤。
    pub manufacturer_company_ids: Vec<u16>,
    /// Additional hard filters over manufacturer data.
    /// 附加的厂商数据硬性过滤器。
    manufacturer_predicates: Vec<Arc<ManufacturerPredicate>>,
    /// Additional hard filters over the full peripheral properties snapshot.
    /// 附加的完整属性硬性过滤器。
    property_predicates: Vec<Arc<PropertiesPredicate>>,
    /// Scan interval between iterations in seconds (default: 2).
    /// 迭代之间的扫描间隔，单位秒（默认：2）。
    pub scan_interval_secs: u64,
}

impl ScanFilter {
    /// Add a name pattern filter (supports prefix matching).
    /// 添加名称模式过滤器（支持前缀匹配）。
    pub fn with_name_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.name_patterns.push(pattern.into());
        self
    }

    /// Add multiple name pattern filters.
    /// 添加多个名称模式过滤器。
    pub fn with_name_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.name_patterns
            .extend(patterns.into_iter().map(|pattern| pattern.into()));
        self
    }

    /// Add an address/device-id pattern filter (supports prefix matching).
    /// 添加地址/设备 ID 模式过滤器（支持前缀匹配）。
    pub fn with_addr_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.addr_patterns.push(pattern.into());
        self
    }

    /// Add multiple address/device-id pattern filters.
    /// 添加多个地址/设备 ID 模式过滤器。
    pub fn with_addr_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.addr_patterns
            .extend(patterns.into_iter().map(|pattern| pattern.into()));
        self
    }

    /// Add a service UUID filter (used in OS scan).
    /// 添加服务 UUID 过滤器（用于操作系统扫描）。
    pub fn with_service_uuid(mut self, uuid: Uuid) -> Self {
        self.service_uuids.push(uuid);
        self
    }

    /// Add multiple service UUID filters.
    /// 添加多个服务 UUID 过滤器。
    pub fn with_service_uuids(mut self, uuids: impl IntoIterator<Item = Uuid>) -> Self {
        self.service_uuids.extend(uuids);
        self
    }

    /// Add a manufacturer company identifier filter.
    /// 添加厂商公司标识符过滤器。
    pub fn with_manufacturer_company_id(mut self, company_id: u16) -> Self {
        self.manufacturer_company_ids.push(company_id);
        self
    }

    /// Add multiple manufacturer company identifier filters.
    /// 添加多个厂商公司标识符过滤器。
    pub fn with_manufacturer_company_ids(
        mut self,
        company_ids: impl IntoIterator<Item = u16>,
    ) -> Self {
        self.manufacturer_company_ids.extend(company_ids);
        self
    }

    /// Add a manufacturer-data predicate filter.
    /// 添加厂商数据断言过滤器。
    pub fn with_manufacturer_data<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&ManufacturerData) -> bool + Send + Sync + 'static,
    {
        self.manufacturer_predicates.push(Arc::new(predicate));
        self
    }

    /// Add an arbitrary properties predicate filter.
    /// 添加任意属性断言过滤器。
    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&PeripheralProperties) -> bool + Send + Sync + 'static,
    {
        self.property_predicates.push(Arc::new(predicate));
        self
    }

    /// Set scan interval between iterations.
    /// 设置迭代之间的扫描间隔。
    pub fn with_scan_interval_secs(mut self, secs: u64) -> Self {
        self.scan_interval_secs = secs;
        self
    }

    /// Check whether a peripheral properties snapshot passes this scan filter.
    /// 检查外设属性快照是否通过此扫描过滤器。
    ///
    /// All filter categories must pass (AND between categories).
    /// 所有过滤类别都必须通过（类别之间是 AND 关系）。
    pub(crate) fn matches_properties(&self, properties: &PeripheralProperties) -> bool {
        // Name / address matching.
        // 名称 / 地址匹配。
        if !self.matches_name_or_address(properties) {
            return false;
        }

        // Manufacturer data matching.
        // 厂商数据匹配。
        if !self.matches_manufacturer(properties.manufacturer_data.as_ref()) {
            return false;
        }

        // Arbitrary property predicates.
        // 任意属性断言。
        self.property_predicates
            .iter()
            .all(|predicate| predicate(properties))
    }

    /// Check name and address patterns (OR within each category, AND between categories).
    /// 检查名称和地址模式（类别内 OR，类别间 AND）。
    fn matches_name_or_address(&self, properties: &PeripheralProperties) -> bool {
        let name = properties.local_name.as_deref().unwrap_or_default();
        let address_or_id = &properties.id;

        let name_has_filters = !self.name_patterns.is_empty();
        let addr_has_filters = !self.addr_patterns.is_empty();

        // Match if any pattern matches (prefix or exact).
        // 任意模式匹配即通过（前缀或精确匹配）。
        let name_matches = self
            .name_patterns
            .iter()
            .any(|pattern| pattern.is_empty() || name.starts_with(pattern) || pattern == name);
        let addr_matches = self.addr_patterns.iter().any(|pattern| {
            pattern.is_empty() || address_or_id.starts_with(pattern) || pattern == address_or_id
        });

        // No filters → pass. Only name filters → name must match. Both → either matches.
        // 无过滤器 → 通过。只有名称过滤器 → 名称须匹配。两者都有 → 任一匹配即可。
        match (name_has_filters, addr_has_filters) {
            (false, false) => true,
            (true, false) => name_matches,
            (false, true) => addr_matches,
            (true, true) => name_matches || addr_matches,
        }
    }

    /// Check manufacturer company IDs and predicates.
    /// 检查厂商公司 ID 和断言。
    fn matches_manufacturer(&self, manufacturer: Option<&ManufacturerData>) -> bool {
        // If company ID filters exist, at least one must match.
        // 如果存在公司 ID 过滤器，至少一个必须匹配。
        if !self.manufacturer_company_ids.is_empty() {
            let Some(manufacturer) = manufacturer else {
                return false;
            };
            if !self
                .manufacturer_company_ids
                .iter()
                .any(|company_id| manufacturer.is_company_id(*company_id))
            {
                return false;
            }
        }

        // All manufacturer predicates must pass.
        // 所有厂商数据断言必须通过。
        self.manufacturer_predicates.iter().all(|predicate| {
            manufacturer
                .as_ref()
                .is_some_and(|manufacturer| predicate(manufacturer))
        })
    }
}

#[cfg(test)]
mod tests;
