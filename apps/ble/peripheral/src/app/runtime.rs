use crate::app::{
    advertising::{AdvertisementData, build_product_advertisement},
    server::{Server, build_server},
    session::run_product_session,
    tasks::custom_task,
};
use embassy_futures::select::select;
use crate::gap::advertising::AdvertisementView;
use trouble_host::prelude::*;

/// App 层运行时聚合 / App-layer runtime bundle.
///
/// 把 `gap::peripheral_loop` 需要的产品入口收敛到一个对象上，
/// 降低 GAP 层对 app 细节的直接依赖。
/// Collects the app-facing entry points needed by `gap::peripheral_loop`
/// so the GAP layer depends on one bundle instead of multiple app details.
pub(crate) struct AppRuntime<'values> {
    server: Server<'values>,
    advertising: AdvertisementData,
}

impl<'values> AppRuntime<'values> {
    pub(crate) fn server(&self) -> &Server<'values> {
        &self.server
    }

    pub(crate) fn advertising_view(&self) -> AdvertisementView<'_> {
        AdvertisementView {
            adv_data: &self.advertising.adv_data[..self.advertising.adv_len],
            scan_data: &self.advertising.scan_data[..self.advertising.scan_len],
        }
    }

    /// 运行一次产品连接会话 / Run one product-specific connected session.
    pub(crate) async fn run_connected_session(
        &self,
        conn: Connection<'_, DefaultPacketPool>,
    ) -> Result<(), Error> {
        let conn = conn.with_attribute_server(self.server())?;
        let a = run_product_session(self.server(), &conn);
        let b = custom_task(self.server(), &conn);
        let _ = select(a, b).await;
        Ok(())
    }
}

/// 构建 app 层运行时对象 / Build the app-layer runtime bundle.
pub(crate) fn build_runtime<'values>() -> Result<AppRuntime<'values>, Error> {
    Ok(AppRuntime {
        server: build_server(),
        advertising: build_product_advertisement()?,
    })
}
