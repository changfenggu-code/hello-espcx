//! Formatting helpers for `Peripheral` and peripheral lists.
//! `Peripheral` 和外设列表的格式化辅助。
//!
//! # Public types / 公开类型
//!
//! | Type | Description |
//! |------|-------------|
//! | [`PeripheralDisplayList`] | Display wrapper for a peripheral list (one device per line). 外设列表的 Display 包装（每行一个设备）。 |
//! | [`PeripheralDisplayExt`] | Extension trait for `display_lines()`. `display_lines()` 的扩展 trait。 |
//!
//! # PeripheralDisplayExt methods / PeripheralDisplayExt 方法
//!
//! | Method | Description |
//! |--------|-------------|
//! | [`PeripheralDisplayExt::display_lines`] | Render peripheral list as newline-separated summaries. 将外设列表渲染为换行分隔的摘要。 |
//!
//! # Display implementations / Display 实现
//!
//! | Type | Output format |
//! |------|--------------|
//! | [`Peripheral`] | `name [id=.., rssi=.., connectable=..]` |
//! | [`PeripheralDisplayList`] | One line per peripheral, newline-separated. 每行一个外设，换行分隔。 |
//!
//! # Usage / 使用方式
//!
//! ```rust,ignore
//! // Single peripheral: one-line summary
//! // 单个外设：单行摘要
//! println!("{peripheral}");
//!
//! // Full ranked list: newline-separated
//! // 完整排序列表：换行分隔
//! println!("{}", ranked.display_lines());
//! ```

use std::fmt;

use super::Peripheral;

/// Display wrapper for a list of peripherals, one device per line.
/// 外设列表的 Display 包装，每个设备占一行。
#[derive(Clone, Copy)]
pub struct PeripheralDisplayList<'a> {
    peripherals: &'a [Peripheral],
}

impl<'a> PeripheralDisplayList<'a> {
    fn new(peripherals: &'a [Peripheral]) -> Self {
        Self { peripherals }
    }
}

/// Convenience helpers for formatting a slice or vec of peripherals.
/// 为 `&[Peripheral]` 和 `Vec<Peripheral>` 提供格式化便捷方法。
pub trait PeripheralDisplayExt {
    /// Render the peripheral list as newline-separated summaries.
    /// 将外设列表渲染为换行分隔的摘要。
    fn display_lines(&self) -> PeripheralDisplayList<'_>;
}

impl PeripheralDisplayExt for [Peripheral] {
    fn display_lines(&self) -> PeripheralDisplayList<'_> {
        PeripheralDisplayList::new(self)
    }
}

impl PeripheralDisplayExt for Vec<Peripheral> {
    fn display_lines(&self) -> PeripheralDisplayList<'_> {
        PeripheralDisplayList::new(self.as_slice())
    }
}

impl fmt::Display for Peripheral {
    /// One-line summary: `local_name [id=.., rssi=.., connectable=..]`.
    /// 单行摘要：`local_name [id=.., rssi=.., connectable=..]`。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = self.local_name().unwrap_or("(unknown)");
        let rssi = self
            .properties()
            .rssi
            .map(|r| r.to_string())
            .unwrap_or_else(|| "?".to_string());
        write!(
            f,
            "{} [id={}, rssi={}, connectable={}]",
            name,
            self.id(),
            rssi,
            self.properties().is_connectable
        )
    }
}

impl fmt::Display for PeripheralDisplayList<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.peripherals.is_empty() {
            return write!(f, "(no peripherals)");
        }

        for (index, peripheral) in self.peripherals.iter().enumerate() {
            if index > 0 {
                writeln!(f)?;
            }
            write!(f, "{peripheral}")?;
        }

        Ok(())
    }
}
