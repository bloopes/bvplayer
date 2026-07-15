//! B 站 API 交互模块。
//!
//! 封装视频信息查询、收藏夹导入、UP 主合集导入及音频流直链解析。
//! 所有公开函数通过 `#[tauri::command]` 暴露给前端调用。

use reqwest::Client;
use serde_json::Value;
use serde::{Deserialize, Serialize};

/// 歌曲 / 视频元数据，贯穿前端播放列表与后端缓存下载。
///
/// `audio_url` 初始为空，播放前通过 JIT（Just-In-Time）方式调用
/// B 站 playurl API 实时获取，避免预取大量链接导致的请求风暴和签名过期。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Song {
    /// B 站视频 BV 号，作为全局唯一标识用于所有 API 查询。
    pub bvid: String,
    /// 视频内分片 / 音轨编号；收藏夹等接口不返回 cid，需后续通过 `fetch_default_cid` 补全。
    #[serde(default)]
    pub cid: u64,

    pub title: String,
    pub author: String,
    pub cover_url: String,
    /// 音频流直链，延迟到播放前一刻通过 `fetch_play_url` 获取。
    pub audio_url: String,
    /// 时长（秒）。
    pub duration: u64,
}

/// 根据 BV 号查询视频信息，自动处理单 P 和多 P 视频。
///
/// 遍历 B 站 view API 返回的 `pages` 数组，每个分 P 生成一条独立的 `Song` 记录。
/// 多 P 视频的标题格式为 `{主标题} - {分P标题}`。
///
/// B 站 API: `https://api.bilibili.com/x/web-interface/view`
#[tauri::command]
pub async fn import_bvid(bvid: String) -> Result<Vec<Song>, String> {
    let client = Client::new();
    let url = format!("https://api.bilibili.com/x/web-interface/view?bvid={}", bvid);
    let json: Value = client.get(&url).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;

    if json["code"].as_i64().unwrap_or(0) != 0 {
        return Err(format!("API错误: {}", json["message"].as_str().unwrap_or("未知")));
    }

    let mut songs = Vec::new();
    let data = &json["data"];
    let base_title = data["title"].as_str().unwrap_or("").to_string();
    let cover_url = data["pic"].as_str().unwrap_or("").to_string();
    let author = data["owner"]["name"].as_str().unwrap_or("未知UP主").to_string();

    // 遍历 pages 数组以支持多 P 视频——每个分 P 有独立的 cid 和时长
    if let Some(pages) = data["pages"].as_array() {
        let is_multi = pages.len() > 1; // 单 P 视频标题不加分 P 后缀
        for page in pages {
            let cid = page["cid"].as_u64().unwrap_or(0);
            let part_title = page["part"].as_str().unwrap_or("").to_string();
            let duration = page["duration"].as_u64().unwrap_or(0);

            let final_title = if is_multi { format!("{} - {}", base_title, part_title) } else { base_title.clone() };

            songs.push(Song {
                bvid: bvid.clone(),
                cid,
                title: final_title,
                author: author.clone(),
                cover_url: cover_url.clone(),
                audio_url: String::new(), // 音频直链延迟到播放前通过 JIT 获取，避免签名过期
                duration,
            });
        }
    }
    Ok(songs)
}

/// JIT 补全：根据 BV 号查询默认 CID（取 `pages[0].cid`）。
///
/// 收藏夹和合集接口不返回 cid 字段，但后续获取音频流必须提供 cid，
/// 因此需要单独调用 view API 补齐。
pub async fn fetch_default_cid(client: &Client, bvid: &str) -> Result<u64, String> {
    let url = format!("https://api.bilibili.com/x/web-interface/view?bvid={}", bvid);
    let json: Value = client.get(&url).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;
    if let Some(pages) = json["data"]["pages"].as_array() {
        if let Some(first) = pages.first() {
            return Ok(first["cid"].as_u64().unwrap_or(0));
        }
    }
    Err("无法获取视频 CID".into())
}

/// JIT 解析：根据 BV 号 + CID 获取音频流直链。
///
/// 调用 B 站 playurl API（`fnval=16` 启用 DASH 格式），
/// 从返回的音频流列表中自动选择最高码率的版本。
///
/// B 站 API: `https://api.bilibili.com/x/player/playurl`
pub async fn fetch_play_url(client: &Client, bvid: &str, cid: u64) -> Result<String, String> {
    let url = format!("https://api.bilibili.com/x/player/playurl?bvid={}&cid={}&fnval=16", bvid, cid);
    let json: Value = client.get(&url).send().await.map_err(|e| e.to_string())?.json().await.map_err(|e| e.to_string())?;

    if let Some(audio_list) = json["data"]["dash"]["audio"].as_array() {
        // 按 bandwidth 降序选最高音质，确保播放体验
        if let Some(best) = audio_list.iter().max_by_key(|a| a["bandwidth"].as_u64().unwrap_or(0)) {
            return Ok(best["baseUrl"].as_str().unwrap_or("").to_string());
        }
    }
    Err("无法提取音频流".into())
}

/// 批量导入公开收藏夹中的所有视频。
///
/// 分页遍历 B 站收藏夹 API（每页 20 条），自动跳过"已失效视频"。
/// 注意：收藏夹接口不返回 `cid`，需在播放前通过 `fetch_default_cid` 补全。
///
/// B 站 API: `https://api.bilibili.com/x/v3/fav/resource/list`
#[tauri::command]
pub async fn import_fav_list(fid: String) -> Result<Vec<Song>, String> {
    let client = reqwest::Client::new();
    let mut all_songs = Vec::new();
    let mut page = 1;

    loop {
        let url = format!(
            "https://api.bilibili.com/x/v3/fav/resource/list?media_id={}&pn={}&ps=20",
            fid, page
        );

        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let json: Value = resp.json().await.map_err(|e| e.to_string())?;

        if json["code"].as_i64().unwrap_or(0) != 0 {
            return Err(format!("API错误: {}", json["message"].as_str().unwrap_or("未知")));
        }

        let medias = json["data"]["medias"].as_array();
        if medias.is_none() || medias.unwrap().is_empty() {
            break;
        }

        for item in medias.unwrap() {
            let title = item["title"].as_str().unwrap_or("").to_string();
            // 已失效视频无法播放且无有效元数据，直接跳过
            if title == "已失效视频" { continue; }

            all_songs.push(Song {
                bvid: item["bvid"].as_str().unwrap_or("").to_string(),
                title,
                cover_url: item["cover"].as_str().unwrap_or("").to_string(),
                duration: item["duration"].as_u64().unwrap_or(0),
                author: item["upper"]["name"].as_str().unwrap_or("未知UP主").to_string(),
                audio_url: String::new(),
                cid: 0, // 收藏夹接口不返回 cid，留空由后续 JIT 补全
            });
        }

        page += 1;
        // 间隔 200ms 避免触发 B 站 API 频率限制
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_songs)
}

/// 批量导入 UP 主视频合集（season）。
///
/// 分页遍历合集 API（每页 30 条）。合集接口同样不返回 `cid` 和 UP 主名，
/// author 字段统一填充为"合集视频"，cid 留待 JIT 补全。
///
/// B 站 API: `https://api.bilibili.com/x/polymer/web-space/seasons_archives_list`
#[tauri::command]
pub async fn import_season_list(sid: String) -> Result<Vec<Song>, String> {
    let client = reqwest::Client::new();
    let mut all_songs = Vec::new();
    let mut page = 1;

    loop {
        let url = format!(
            "https://api.bilibili.com/x/polymer/web-space/seasons_archives_list?season_id={}&page_num={}&page_size=30",
            sid, page
        );

        let resp = client.get(&url).send().await.map_err(|e| e.to_string())?;
        let json: Value = resp.json().await.map_err(|e| e.to_string())?;

        if json["code"].as_i64().unwrap_or(0) != 0 {
            return Err(format!("API错误: {}", json["message"].as_str().unwrap_or("未知")));
        }

        let archives = json["data"]["archives"].as_array();
        if archives.is_none() || archives.unwrap().is_empty() {
            break;
        }

        for item in archives.unwrap() {
            all_songs.push(Song {
                bvid: item["bvid"].as_str().unwrap_or("").to_string(),
                title: item["title"].as_str().unwrap_or("").to_string(),
                cover_url: item["pic"].as_str().unwrap_or("").to_string(),
                duration: item["duration"].as_u64().unwrap_or(0),
                author: "合集视频".to_string(), // 合集接口不返回 UP 主信息，统一占位
                audio_url: String::new(),
                cid: 0,
            });
        }

        page += 1;
        // 间隔 200ms 避免触发 B 站 API 频率限制
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }

    Ok(all_songs)
}
