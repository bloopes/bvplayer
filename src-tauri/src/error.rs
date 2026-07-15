//! 统一错误类型定义模块。
//!
//! 使用 thiserror 将各底层依赖的错误统一转换为 `AppError`，
//! 避免在业务代码中使用 `Result<T, String>` 丢失错误上下文。

use thiserror::Error;

/// 应用层错误枚举，收敛所有外部依赖的失败路径。
///
/// 每种变体对应一类底层故障，通过 `#[from]` 实现自动类型转换，
/// 调用方可统一使用 `?` 运算符传播错误。
#[derive(Error, Debug)]
pub enum AppError {
    /// HTTP 请求失败（DNS 解析、连接超时、TLS 握手等）。
    #[error("网络请求断裂: {0}")]
    Network(#[from] reqwest::Error),

    /// 文件读写失败（权限不足、磁盘满、路径非法等）。
    #[error("本地文件系统故障: {0}")]
    Io(#[from] std::io::Error),

    /// B 站 API 返回的 JSON 结构与预期模型不匹配。
    #[error("API 数据模具解析失败: {0}")]
    Parse(#[from] serde_json::Error),

    /// 音频解码器或底层播放引擎抛出的错误，用字符串携带详情。
    #[error("底层音频引擎异常: {0}")]
    Audio(String),
}

/// 应用全局返回类型别名，统一使用 `AppError` 替代裸 `String` 错误。
pub type AppResult<T> = Result<T, AppError>;