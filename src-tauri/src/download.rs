//! 视频流下载与缓存模块。
//!
//! 将 B 站视频流的 m4s 分片下载到系统临时目录，
//! 避开 Tauri 文件监听器对项目目录的变更检测。
use std::fs;
use std::path::PathBuf;
use tokio::io::AsyncWriteExt;

/// 获取缓存根目录（系统临时文件夹下的 `bvplayer_cache`）。
///
/// 放在临时目录而非项目目录，是为了彻底避开 Tauri 的文件监听器——
/// 项目目录内的任意写入都会触发 Tauri 的热重载 / dev 检测。
pub fn get_cache_dir() -> PathBuf {
    let mut path = std::env::temp_dir();
    path.push("bvplayer_cache");
    // 首次运行时临时目录下可能还没有此子目录，需主动创建
    if !path.exists() {
        let _ = fs::create_dir_all(&path);
    }
    path
}

/// 根据 BV 号和 CID 生成缓存文件路径。
///
/// 命名规则：`{BV号}_{CID}.m4s`，确保同一视频不同分片不会互相覆盖。
pub fn get_cache_path(bvid: &str, cid: u64) -> String {
    get_cache_dir()
        .join(format!("{}_{}.m4s", bvid, cid))
        .to_string_lossy()
        .to_string()
}

/// 检查指定分片是否已缓存到本地，避免重复下载。
pub fn is_cached(bvid: &str, cid: u64) -> bool {
    std::path::Path::new(&get_cache_path(bvid, cid)).exists()
}

/// 将远程视频流下载到本地缓存。
///
/// 已缓存则直接返回路径；否则发起 HTTP GET，将响应体写入文件。
/// 请求需伪装浏览器身份以通过 B 站防盗链校验。
///
/// # 参数
/// - `client`: 复用的 HTTP 客户端实例（避免频繁 TCP 握手）
/// - `url`: 视频流直链地址
/// - `bvid`: B 站视频 BV 号
/// - `cid`: 视频分片编号
///
/// # 返回值
/// - `Ok(String)`: 缓存文件的本地完整路径
/// - `Err(String)`: 下载或写入失败的错误描述
pub async fn stream_to_disk(
    client: &reqwest::Client,
    url: &str,
    bvid: &str,
    cid: u64,
) -> Result<String, String> {
    let path = get_cache_path(bvid, cid);
    if is_cached(bvid, cid) {
        return Ok(path);
    }

    let response = client
        .get(url)
        // B 站 CDN 会校验 Referer 和 UA 头，缺少任一个都会返回 403 Forbidden
        .header("Referer", "https://www.bilibili.com")
        .header(
            "User-Agent",
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36",
        )
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let bytes = response.bytes().await.map_err(|e| e.to_string())?;

    let mut file = tokio::fs::File::create(&path)
        .await
        .map_err(|e| format!("缓存创建失败: {}", e))?;
    file.write_all(&bytes)
        .await
        .map_err(|e| format!("缓存写入失败: {}", e))?;

    Ok(path)
}
