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
//! | [`ScanFilter::with_scan_interval_secs`] | Set scan interval in seconds. 设置扫描间隔（秒）。 |

use bluest::Uuid;

/// Scan filter for discovering peripherals.
/// 用于发现外设的扫描过滤器。
///
/// Filters can be combined (all conditions are OR'd within each category):
/// 可以组合过滤器（每个类别内的所有条件都是 OR 关系）：
/// - Empty `name_patterns`/`addr_patterns` matches all
/// - 空的 `name_patterns`/`addr_patterns` 匹配所有
/// - Non-empty filters match if device matches any pattern (prefix supported)
/// - 非空过滤器在设备匹配任意模式时匹配（支持前缀匹配）
///
/// Service UUIDs are used for OS-level filtering during scan.
/// 服务 UUID 用于扫描期间的操作系统级别过滤。
#[derive(Default, Clone)]
pub struct ScanFilter {
    /// Filter by peripheral name patterns (OR'd, prefix matching supported)
    /// 按外设名称模式过滤（OR 关系，支持前缀匹配）
    pub name_patterns: Vec<String>,
    /// Filter by address patterns in format "XXXXXXXXXXXX" (OR'd, prefix supported)
    /// 按地址模式过滤，格式为 "XXXXXXXXXXXX"（OR 关系，支持前缀）
    pub addr_patterns: Vec<String>,
    /// Filter by service UUIDs (OS-level scan filter)
    /// 按服务 UUID 过滤（操作系统级别扫描过滤器）
    pub service_uuids: Vec<Uuid>,
    /// Scan interval between iterations in seconds (default: 2)
    /// 迭代之间的扫描间隔，单位秒（默认：2）
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
            .extend(patterns.into_iter().map(|n| n.into()));
        self
    }

    /// Add an address pattern filter (supports prefix matching).
    /// 添加地址模式过滤器（支持前缀匹配）。
    pub fn with_addr_pattern(mut self, pattern: impl Into<String>) -> Self {
        self.addr_patterns.push(pattern.into());
        self
    }

    /// Add multiple address pattern filters.
    /// 添加多个地址模式过滤器。
    pub fn with_addr_patterns(
        mut self,
        patterns: impl IntoIterator<Item = impl Into<String>>,
    ) -> Self {
        self.addr_patterns
            .extend(patterns.into_iter().map(|a| a.into()));
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

    /// Set scan interval between iterations.
    /// 设置迭代之间的扫描间隔。
    pub fn with_scan_interval_secs(mut self, secs: u64) -> Self {
        self.scan_interval_secs = secs;
        self
    }

    /// Check if a device matches this filter by name or address.
    /// 检查设备是否通过名称或地址匹配此过滤器。
    ///
    /// Uses OR logic: matches if name matches OR address matches.
    /// 使用 OR 逻辑：名称匹配或地址匹配即视为匹配。
    /// Pattern matching supports prefix matching (e.g., "SmartBulb-" matches "SmartBulb-A1B2C3").
    /// 模式匹配支持前缀匹配（例如，"SmartBulb-" 匹配 "SmartBulb-A1B2C3"）。
    pub(crate) fn matches(&self, name: &str, address: &str) -> bool {
        let name_matches = self.name_patterns.is_empty()
            || self
                .name_patterns
                .iter()
                .any(|p| p.is_empty() || name.starts_with(p) || *p == name);

        let addr_matches = self.addr_patterns.is_empty()
            || self
                .addr_patterns
                .iter()
                .any(|p| p.is_empty() || address.starts_with(p) || *p == address);

        name_matches || addr_matches
    }
}
