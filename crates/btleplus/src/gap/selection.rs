//! Selector builder for ranking and choosing one peripheral from many scan results.
//! 选择器构建器：从多个扫描结果中排序并选择一个外设。
//!
//! # Public types / 公开类型
//!
//! | Type | Description |
//! |------|-------------|
//! | [`Selector`] | Builder for soft preferences and hard filters. 软偏好和硬性过滤的构建器。 |
//! | [`PeripheralSelectionExt`] | Extension trait for applying a selector to a peripheral collection. 将选择器应用到外设集合的扩展 trait。 |
//!
//! # Selector methods / Selector 方法
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`Selector::prefer_connectable`] | Soft-prefer connectable devices. 软偏好：优先可连接设备。 |
//! | [`Selector::prefer_strongest_signal`] | Soft-prefer strongest signal (RSSI). 软偏好：优先信号最强。 |
//! | [`Selector::prefer_id`] | Soft-prefer a specific device id. 软偏好：优先指定设备 ID。 |
//! | [`Selector::prefer_local_name`] | Soft-prefer a specific local name. 软偏好：优先指定广播名称。 |
//! | [`Selector::prefer_manufacturer_company_id`] | Soft-prefer a manufacturer company ID. 软偏好：优先指定厂商公司 ID。 |
//! | [`Selector::prefer_manufacturer_data`] | Soft-prefer by manufacturer data predicate. 软偏好：按厂商数据断言。 |
//! | [`Selector::filter`] | Add a hard-filter predicate. 添加硬性过滤断言。 |
//! | [`Selector::select`] | Return the best-matching peripheral. 返回最佳匹配的外设。 |
//! | [`Selector::rank`] | Return all peripherals in ranked order. 返回按偏好排序的全部外设。 |
//!
//! # Extension trait methods / 扩展 trait 方法
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`PeripheralSelectionExt::select_with`] | Apply selector and return the best one. 应用选择器，返回最佳设备。 |
//! | [`PeripheralSelectionExt::rank_with`] | Apply selector and return all ranked. 应用选择器，返回全部排名结果。 |
//!
//! # How it works / 工作原理
//!
//! `prefer_*` methods are soft preferences. They only affect ordering, and the
//! chain order is the priority order.
//! `prefer_*` 方法是软偏好，只影响排序，链式调用顺序即为优先级顺序。

use crate::error::BtleplusError;

use super::peripheral::{
    ManufacturerData, ManufacturerPredicate, Peripheral, PeripheralProperties, PropertiesPredicate,
};

/// Convenience helpers for applying a selector to a peripheral collection.
/// 将选择器应用到外设集合的便捷方法。
pub trait PeripheralSelectionExt {
    /// Select the best peripheral using the provided selector.
    /// 使用提供的选择器选出最佳外设。
    fn select_with(&self, selector: &Selector) -> Result<Peripheral, BtleplusError>;

    /// Rank all peripherals using the provided selector.
    /// 使用提供的选择器对所有外设排序。
    fn rank_with(&self, selector: &Selector) -> Result<Vec<Peripheral>, BtleplusError>;
}

// Trait impl for slices: delegate directly to Selector methods.
// 切片类型的 trait 实现：直接委托给 Selector 的方法。
impl PeripheralSelectionExt for [Peripheral] {
    fn select_with(&self, selector: &Selector) -> Result<Peripheral, BtleplusError> {
        selector.select(self)
    }

    fn rank_with(&self, selector: &Selector) -> Result<Vec<Peripheral>, BtleplusError> {
        selector.rank(self)
    }
}

// Trait impl for Vec: borrow as slice and reuse the slice impl above.
// Vec 类型的 trait 实现：借为切片，复用上面的切片实现。
impl PeripheralSelectionExt for Vec<Peripheral> {
    fn select_with(&self, selector: &Selector) -> Result<Peripheral, BtleplusError> {
        self.as_slice().select_with(selector)
    }

    fn rank_with(&self, selector: &Selector) -> Result<Vec<Peripheral>, BtleplusError> {
        self.as_slice().rank_with(selector)
    }
}

/// Builder that ranks peripherals and optionally applies post-discovery filters.
/// 外设排序构建器，可选择性地应用发现后过滤器。
#[derive(Default)]
pub struct Selector {
    /// Ordered soft preferences; earlier entries have higher priority.
    /// 有序的软偏好列表；靠前的条目优先级更高。
    preferences: Vec<Preference>,
    /// Hard filters applied after discovery and before ranking.
    /// 在发现之后、排序之前应用的硬性过滤器。
    filters: Vec<Box<PropertiesPredicate>>,
}

impl Selector {
    /// Soft-prefer connectable devices over non-connectable ones.
    /// 软偏好：优先选择可连接的设备。
    pub fn prefer_connectable(mut self) -> Self {
        self.preferences.push(Preference::Connectable);
        self
    }

    /// Soft-prefer the device with the strongest signal.
    /// 软偏好：优先选择信号最强的设备。
    pub fn prefer_strongest_signal(mut self) -> Self {
        self.preferences.push(Preference::StrongestSignal);
        self
    }

    /// Soft-prefer a device with the exact given platform id.
    /// 软偏好：优先选择指定平台 ID 的设备。
    pub fn prefer_id(mut self, id: impl Into<String>) -> Self {
        self.preferences.push(Preference::Id(id.into()));
        self
    }

    /// Soft-prefer a device with the exact given advertised local name.
    /// 软偏好：优先选择指定广播名称的设备。
    pub fn prefer_local_name(mut self, name: impl Into<String>) -> Self {
        self.preferences.push(Preference::LocalName(name.into()));
        self
    }

    /// Soft-prefer devices with the given manufacturer company identifier.
    /// 软偏好：优先选择指定厂商公司 ID 的设备。
    pub fn prefer_manufacturer_company_id(self, company_id: u16) -> Self {
        self.prefer_manufacturer_data(move |data| data.is_company_id(company_id))
    }

    /// Soft-prefer devices whose manufacturer data matches the given predicate.
    /// 软偏好：优先选择厂商数据匹配指定断言的设备。
    pub fn prefer_manufacturer_data<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&ManufacturerData) -> bool + Send + Sync + 'static,
    {
        self.preferences
            .push(Preference::Manufacturer(Box::new(predicate)));
        self
    }

    /// Add a post-discovery hard-filter predicate.
    /// 添加发现后的硬性过滤断言。
    pub fn filter<F>(mut self, predicate: F) -> Self
    where
        F: Fn(&PeripheralProperties) -> bool + Send + Sync + 'static,
    {
        self.filters.push(Box::new(predicate));
        self
    }

    /// Run the selector pipeline and return the best-matching peripheral.
    /// 执行选择器流水线，返回最佳匹配的外设。
    pub fn select(&self, peripherals: &[Peripheral]) -> Result<Peripheral, BtleplusError> {
        self.select_ref(peripherals).cloned()
    }

    /// Run the selector pipeline and return all surviving peripherals in ranked order.
    /// 执行选择器流水线，返回所有通过过滤的外设（按偏好排序）。
    pub fn rank(&self, peripherals: &[Peripheral]) -> Result<Vec<Peripheral>, BtleplusError> {
        self.rank_ref(peripherals)
            .map(|ranked| ranked.into_iter().cloned().collect::<Vec<_>>())
    }

    // Internal: rank then take the first (best) candidate.
    // 内部方法：排序后取第一个（最佳）候选者。
    fn select_ref<'a, T>(&self, input: &'a [T]) -> Result<&'a T, BtleplusError>
    where
        T: Candidate,
    {
        self.rank_ref(input)?
            .into_iter()
            .next()
            .ok_or_else(|| BtleplusError::SelectionFailed("no peripherals available".to_string()))
    }

    // Core ranking logic: filter out non-matching candidates, then sort by preferences.
    // 核心排序逻辑：过滤掉不匹配的候选者，然后按偏好排序。
    fn rank_ref<'a, T>(&self, input: &'a [T]) -> Result<Vec<&'a T>, BtleplusError>
    where
        T: Candidate,
    {
        // Keep only candidates that pass all hard filters.
        // 仅保留通过所有硬性过滤器的候选者。
        let mut candidates: Vec<&T> = input
            .iter()
            .filter(|candidate| self.matches_filters(*candidate))
            .collect();

        if candidates.is_empty() {
            return Err(BtleplusError::SelectionFailed(
                "no peripherals matched selector".to_string(),
            ));
        }

        // Sort by soft preferences, with device id as tiebreaker.
        // 按软偏好排序，设备 id 作为平局决胜。
        candidates.sort_by(|left, right| self.compare(*left, *right));
        Ok(candidates)
    }

    // Check if a peripheral passes all hard filter predicates.
    // 检查外设是否通过所有硬性过滤断言。
    fn matches_filters<T: Candidate>(&self, peripheral: &T) -> bool {
        self.filters
            .iter()
            .all(|predicate| predicate(peripheral.properties()))
    }

    // Compare two candidates by iterating preferences in priority order.
    // Returns the first non-equal comparison, or falls back to device id ordering.
    // 按优先级顺序遍历偏好来比较两个候选者。
    // 返回第一个非相等的比较结果，若无则回退到设备 id 排序。
    fn compare<T: Candidate>(&self, left: &T, right: &T) -> core::cmp::Ordering {
        self.preferences
            .iter()
            .map(|preference| preference.compare(left.properties(), right.properties()))
            .find(|ordering| !ordering.is_eq())
            .unwrap_or(core::cmp::Ordering::Equal)
            .then_with(|| left.id().cmp(right.id()))
    }
}

/// Internal abstraction shared by real peripherals and test fakes.
/// Allows the selector to operate uniformly on both.
/// 内部抽象：真实外设和测试替身共用，让选择器统一操作。
trait Candidate {
    /// Device identifier string.
    /// 设备标识符字符串。
    fn id(&self) -> &str;

    /// Peripheral properties captured during scanning.
    /// 扫描时捕获的外设属性。
    fn properties(&self) -> &PeripheralProperties;
}

impl Candidate for Peripheral {
    fn id(&self) -> &str {
        Peripheral::id(self)
    }

    fn properties(&self) -> &PeripheralProperties {
        Peripheral::properties(self)
    }
}

/// Soft preference kinds used for pairwise comparison during ranking.
/// 排序时用于两两比较的软偏好种类。
enum Preference {
    /// Prefer connectable devices.
    /// 优先可连接的设备。
    Connectable,
    /// Prefer the device with the highest RSSI.
    /// 优先信号最强的设备（最高 RSSI）。
    StrongestSignal,
    /// Prefer the device whose platform id matches exactly.
    /// 优先平台 ID 完全匹配的设备。
    Id(String),
    /// Prefer the device whose advertised local name matches exactly.
    /// 优先广播名称完全匹配的设备。
    LocalName(String),
    /// Prefer the device whose manufacturer data satisfies the predicate.
    /// 优先厂商数据满足断言的设备。
    Manufacturer(Box<ManufacturerPredicate>),
}

impl Preference {
    /// Compare two peripherals on this single preference dimension.
    /// Returns `Ordering::Greater` when `right` is preferred over `left`.
    /// 按此偏好维度比较两个外设。
    /// 返回 `Ordering::Greater` 表示 `right` 优于 `left`。
    fn compare(
        &self,
        left: &PeripheralProperties,
        right: &PeripheralProperties,
    ) -> core::cmp::Ordering {
        match self {
            // Compare by connectable flag (true > false).
            // 按可连接标志比较（true > false）。
            Self::Connectable => right.is_connectable.cmp(&left.is_connectable),
            // Compare by RSSI; missing RSSI treated as minimum.
            // 按 RSSI 比较；缺失时视为最小值。
            Self::StrongestSignal => right
                .rssi
                .unwrap_or(i16::MIN)
                .cmp(&left.rssi.unwrap_or(i16::MIN)),
            // Exact-match on platform id.
            // 平台 ID 精确匹配。
            Self::Id(id) => (right.id == *id).cmp(&(left.id == *id)),
            // Exact-match on advertised local name.
            // 广播名称精确匹配。
            Self::LocalName(name) => (right.local_name.as_deref() == Some(name.as_str()))
                .cmp(&(left.local_name.as_deref() == Some(name.as_str()))),
            // Match when manufacturer data exists and satisfies the predicate.
            // 厂商数据存在且满足断言时匹配。
            Self::Manufacturer(predicate) => right
                .manufacturer_data
                .as_ref()
                .is_some_and(predicate)
                .cmp(
                    &left
                        .manufacturer_data
                        .as_ref()
                        .is_some_and(predicate),
                ),
        }
    }
}

#[cfg(test)]
mod tests;
