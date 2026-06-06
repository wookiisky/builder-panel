//! 本地 bridge 抽象边界。

use std::time::Duration;

/// 本地 bridge client 抽象。
pub trait LocalBridgeClientPort {
    /// request 类型由 adapter 绑定。
    type Request;
    /// response 类型由 adapter 绑定。
    type Response;
    /// error 类型由 adapter 绑定。
    type Error;

    /// 发送本地 bridge request 并等待可选 response。
    fn send_request(
        &self,
        request: &Self::Request,
        timeout: Duration,
    ) -> Result<Option<Self::Response>, Self::Error>;
}
