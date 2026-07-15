use serde::Serialize;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AppError {
    #[error("网络请求断裂: {0}")]
    Network(#[from] reqwest::Error),

    #[error("本地文件系统故障: {0}")]
    Io(#[from] std::io::Error),

    #[error("API 数据模具解析失败: {0}")]
    Parse(#[from] serde_json::Error),

    // 📍 新增：用于 B 站 API 返回 code != 0 时的业务错误
    #[error("B站API业务错误: {0}")]
    Api(String),

    #[error("底层音频引擎异常: {0}")]
    Audio(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type AppResult<T> = Result<T, AppError>;