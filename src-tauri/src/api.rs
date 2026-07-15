use reqwest::Client;
use serde_json::Value;
use crate::models::Song; // 📍 核心：从 models 引入 Song，而不是在当前文件定义
use crate::error::{AppError, AppResult}; // 📍 核心：引入统一错误处理

#[tauri::command]
pub async fn import_bvid(bvid: String) -> AppResult<Vec<Song>> {
    let client = Client::new();
    let url = format!("https://api.bilibili.com/x/web-interface/view?bvid={}", bvid);
    
    // 📍 优化：直接用 ? 冒泡网络和解析错误
    let json: Value = client.get(&url).send().await?.json().await?;

    if json["code"].as_i64().unwrap_or(0) != 0 {
        // 📍 优化：使用 AppError::Api 抛出业务错误（这就消除了之前的警告）
        return Err(AppError::Api(json["message"].as_str().unwrap_or("未知").to_string()));
    }

    let mut songs = Vec::new();
    let data = &json["data"];
    let base_title = data["title"].as_str().unwrap_or("").to_string();
    let cover_url = data["pic"].as_str().unwrap_or("").to_string();
    let author = data["owner"]["name"].as_str().unwrap_or("未知UP主").to_string();

    if let Some(pages) = data["pages"].as_array() {
        let is_multi = pages.len() > 1;
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
                audio_url: String::new(),
                duration,
            });
        }
    }
    Ok(songs)
}

pub async fn fetch_default_cid(client: &Client, bvid: &str) -> AppResult<u64> {
    let url = format!("https://api.bilibili.com/x/web-interface/view?bvid={}", bvid);
    let json: Value = client.get(&url).send().await?.json().await?;
    
    if let Some(pages) = json["data"]["pages"].as_array() {
        if let Some(first) = pages.first() {
            return Ok(first["cid"].as_u64().unwrap_or(0));
        }
    }
    Err(AppError::Api("无法获取视频 CID".into()))
}

pub async fn fetch_play_url(client: &Client, bvid: &str, cid: u64) -> AppResult<String> {
    let url = format!("https://api.bilibili.com/x/player/playurl?bvid={}&cid={}&fnval=16", bvid, cid);
    let json: Value = client.get(&url).send().await?.json().await?;

    if let Some(audio_list) = json["data"]["dash"]["audio"].as_array() {
        if let Some(best) = audio_list.iter().max_by_key(|a| a["bandwidth"].as_u64().unwrap_or(0)) {
            return Ok(best["baseUrl"].as_str().unwrap_or("").to_string());
        }
    }
    Err(AppError::Api("无法提取音频流".into()))
}

#[tauri::command]
pub async fn import_fav_list(fid: String) -> AppResult<Vec<Song>> {
    let client = Client::new();
    let mut all_songs = Vec::new();
    let mut page = 1;

    loop {
        let url = format!("https://api.bilibili.com/x/v3/fav/resource/list?media_id={}&pn={}&ps=20", fid, page);
        let json: Value = client.get(&url).send().await?.json().await?;

        if json["code"].as_i64().unwrap_or(0) != 0 {
            return Err(AppError::Api(json["message"].as_str().unwrap_or("未知").to_string()));
        }

        // 📍 优化：安全解包
        let Some(medias) = json["data"]["medias"].as_array() else { break; };
        if medias.is_empty() { break; }

        for item in medias {
            let title = item["title"].as_str().unwrap_or("").to_string();
            if title == "已失效视频" { continue; }

            all_songs.push(Song {
                bvid: item["bvid"].as_str().unwrap_or("").to_string(),
                title,
                cover_url: item["cover"].as_str().unwrap_or("").to_string(),
                duration: item["duration"].as_u64().unwrap_or(0),
                author: item["upper"]["name"].as_str().unwrap_or("未知UP主").to_string(),
                audio_url: String::new(),
                cid: 0,
            });
        }
        page += 1;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Ok(all_songs)
}

#[tauri::command]
pub async fn import_season_list(sid: String) -> AppResult<Vec<Song>> {
    let client = Client::new();
    let mut all_songs = Vec::new();
    let mut page = 1;

    loop {
        let url = format!("https://api.bilibili.com/x/polymer/web-space/seasons_archives_list?season_id={}&page_num={}&page_size=30", sid, page);
        let json: Value = client.get(&url).send().await?.json().await?;

        if json["code"].as_i64().unwrap_or(0) != 0 {
            return Err(AppError::Api(json["message"].as_str().unwrap_or("未知").to_string()));
        }

        // 📍 优化：彻底消灭了 unwrap().is_empty()
        let Some(archives) = json["data"]["archives"].as_array() else { break; };
        if archives.is_empty() { break; }

        for item in archives {
            all_songs.push(Song {
                bvid: item["bvid"].as_str().unwrap_or("").to_string(),
                title: item["title"].as_str().unwrap_or("").to_string(),
                cover_url: item["pic"].as_str().unwrap_or("").to_string(),
                duration: item["duration"].as_u64().unwrap_or(0),
                author: "合集视频".to_string(),
                audio_url: String::new(),
                cid: 0,
            });
        }
        page += 1;
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    }
    Ok(all_songs)
}