# easyble

基于 `trouble-host` 的分阶段 BLE 外设辅助库。

## 概览

`easyble` 将外设侧 BLE 概念拆成两层：

- `gap`：协议栈初始化、协议栈驱动、广播
- `gatt`：AttributeServer 绑定、单连接会话驱动

推荐的生命周期主路径：

```text
gap::init
  -> gap::advertising
  -> gatt::connected
  -> gatt::session
  -> app 自己处理 disconnected
  -> gap::advertising ...
```

本库有意不接管外层生命周期循环。重连策略、致命错误策略、产品任务和
session 结束后的处理，都留在 app 层。

## 快速开始

```rust
use embassy_futures::join::join;

// 1. 初始化 BLE 协议栈，拿到可复用的 peripheral 和 runner
let easyble::gap::InitializedStack {
    mut peripheral,
    runner,
} = easyble::gap::init::<_, 1, 2>(
    controller,
    easyble::gap::InitConfig {
        address: PERIPHERAL_ADDRESS,
    },
);

// 2. 构建 app 自己的广播载荷和 GATT server
let advertisement = build_advertisement()?;
let server = build_server()?;

join(
    async {
        // 3. 并行驱动底层协议栈任务
        easyble::gap::run_stack(runner).await?;
        Ok::<(), _>(())
    },
    async {
        loop {
            // 4. 进入广播阶段，等待 Central 连接
            let conn =
                easyble::gap::advertising(&mut peripheral, advertisement.as_view()).await?;
            // 5. 连接建立后，把原始连接绑定到 AttributeServer
            let gatt = easyble::gatt::connected(conn, server)?;
            // 6. 运行单连接 GATT 会话，被动处理一个个 GATT 事件
            easyble::gatt::session(&gatt, |event| {
                // 处理一个 GATT 事件
            })
            .await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), _>(())
    },
)
.await;
```

## 调用路径

推荐的 app 自主管理生命周期路径：

```text
main.rs
  -> build_advertisement
  -> build_server
  -> gap::init
  -> gap::run_stack
  -> gap::advertising
  -> gatt::connected
  -> gatt::session
  -> app::custom_task
  -> app 自己处理 disconnected
  -> gap::advertising ...
```

## 一句话记忆

- `gap/init.rs`：先把栈搭起来
- `gap/advertising.rs`：广播一次并等来一个连接
- `gatt/connected.rs`：把原始连接绑到 `AttributeServer`
- `gatt/session.rs`：跑被动 GATT 事件循环
- `gap/mod.rs`：导出 GAP 阶段接口
- `gatt/mod.rs`：导出 GATT 阶段接口
- `lib.rs`：暴露 crate 结构

## 公开 API

对外表面是模块化的：

```rust
pub mod gap;
pub mod gatt;
```

推荐用模块路径，而不是根模块平铺导出：

```rust
easyble::gap::InitConfig
easyble::gap::InitializedStack
easyble::gap::AdvertisementData
easyble::gap::advertising(...)
easyble::gatt::connected(...)
easyble::gatt::session(...)
```

## GAP 层

### `InitConfig`

初始化阶段的 host 配置：

```rust
pub struct InitConfig {
    pub address: [u8; 6],
}
```

当前负责的范围：

- 固定 / 随机 BLE 地址设置
- host 资源分配
- stack 构建

### `InitializedStack`

`init` 阶段的返回结果：

```rust
pub struct InitializedStack<C: Controller + 'static> {
    pub peripheral: Peripheral<'static, C, DefaultPacketPool>,
    pub runner: Runner<'static, C, DefaultPacketPool>,
}
```

app 通常会把这两个成员分开使用：

- `runner`：交给 `gap::run_stack`
- `peripheral`：在多轮 `gap::advertising` 中复用

### `init`

构建外设侧 BLE host stack：

```rust
pub fn init<C, const CONN: usize, const L2CAP: usize>(
    controller: C,
    config: InitConfig,
) -> InitializedStack<C>
where
    C: Controller + 'static
```

重要说明：

- `HostResources` 和 `Stack` 会通过 `Box::leak` 泄漏
- 这是有意为之，用来满足嵌入式固件里后续绑定 server 所需的生命周期

### `run_stack`

驱动底层 `trouble-host` runner 任务：

```rust
pub async fn run_stack<C: Controller + 'static>(
    runner: Runner<'static, C, DefaultPacketPool>,
) -> Result<(), BleHostError<C::Error>>
```

推荐用法：

- 和 app 自己的生命周期循环并行运行
- 如果产品没有更复杂的恢复策略，失败通常按 fatal 处理

### `AdvertisementData`

app 持有的广播载荷缓冲：

```rust
pub struct AdvertisementData {
    pub adv_data: [u8; 31],
    pub adv_len: usize,
    pub scan_data: [u8; 31],
    pub scan_len: usize,
}
```

辅助方法：

```rust
pub fn as_view(&self) -> AdvertisementView<'_>
```

### `AdvertisementView`

单次广播尝试时借用的广播视图：

```rust
pub struct AdvertisementView<'a> {
    pub adv_data: &'a [u8],
    pub scan_data: &'a [u8],
}
```

### `advertising`

执行一次广播阶段，并等待一个 Central 连上来：

```rust
pub async fn advertising<'stack, C: Controller>(
    peripheral: &mut Peripheral<'stack, C, DefaultPacketPool>,
    data: AdvertisementView<'_>,
) -> Result<Connection<'stack, DefaultPacketPool>, BleHostError<C::Error>>
```

行为边界：

- 启动 connectable advertising
- 等待 `accept()`
- 返回一个原始 BLE `Connection`
- 不自动绑定 GATT

## GATT 层

### `connected`

把一个已接受的原始 BLE 连接绑定到 `AttributeServer`：

```rust
pub fn connected<
    'stack,
    'server,
    'values,
    M: RawMutex,
    const ATT_MAX: usize,
    const CCCD_MAX: usize,
    const CONN_MAX: usize,
>(
    conn: Connection<'stack, DefaultPacketPool>,
    server: &'server AttributeServer<'values, M, DefaultPacketPool, ATT_MAX, CCCD_MAX, CONN_MAX>,
) -> Result<GattConnection<'stack, 'server, DefaultPacketPool>, Error>
```

这个阶段的意义：

- 它是从 GAP 生命周期切到 GATT 生命周期的转场点
- 从这里开始，app 拿到的是 `GattConnection`

### `session`

驱动单连接内的被动 GATT 事件循环：

```rust
pub async fn session<P, F>(
    conn: &GattConnection<'_, '_, P>,
    on_event: F,
) -> Result<(), Error>
where
    P: PacketPool,
    F: for<'stack, 'server> FnMut(&GattEvent<'stack, 'server, P>)
```

行为边界：

- 持续等待连接上的 GATT 事件
- 通过 `on_event` 回调把事件交给调用方
- 自动 `accept` 并发送 GATT 响应
- 连接断开后退出

重要边界：

- `session(...)` 只负责被动 GATT 事件
- 主动产品任务仍然应该留在 app 层并行运行

## 设计边界

`easyble` 适合负责的内容：

- stack setup
- 单次广播尝试
- 原始连接到 GATT 连接的绑定
- 被动 GATT 事件循环机制

`easyble` 不应该过早负责的内容：

- 产品级广播语义
- 具体服务定义
- 产品特定的 GATT 事件处理
- 电量推送 / echo 回传 / bulk 流这类主动任务
- 外层重连循环
- disconnected 策略

## 与 `hello-ble-peripheral` 配合的例子

```rust
let advertisement = build_advertisement()?;
let server = build_server()?;

let easyble::gap::InitializedStack {
    mut peripheral,
    runner,
} = easyble::gap::init::<_, 1, 2>(
    controller,
    easyble::gap::InitConfig {
        address: PERIPHERAL_ADDRESS,
    },
);

join(
    async {
        easyble::gap::run_stack(runner).await?;
        Ok::<(), _>(())
    },
    async {
        loop {
            let conn =
                easyble::gap::advertising(&mut peripheral, advertisement.as_view()).await?;
            let gatt_conn = easyble::gatt::connected(conn, server)?;

            join(
                run_product_session(&gatt_conn, server),
                custom_task(&gatt_conn, server),
            )
            .await;
        }
    },
)
.await;
```

## 备注

1. `easyble` 是面向外设侧的，不是 central/client 库。
2. 这个 crate 的主模型是生命周期流，不是 central 风格的“发现对象流”。
3. 外层重连循环有意留在 app 层。
4. `disconnected` 目前仍然由 app 自己处理，而不是库内阶段。
5. 现在的 API 故意保持窄接口，更适合嵌入式固件集成。
