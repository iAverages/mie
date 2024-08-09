use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Media {
    pub id: i32,
    pub url: String,
    pub actual_source: Option<String>,
    pub original_source: String,

    pub size: i32,
    pub file_type: String,
    pub meta: serde_json::Value,
    pub uploader: String,

    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,
}

#[derive(Debug, Clone)]
pub struct AddMedia {
    pub url: String,
    pub actual_source: Option<String>,
    pub original_source: String,

    pub size: i32,
    pub file_type: String,
    pub meta: serde_json::Value,
    pub uploader: String,
}

pub struct DownloadedVideo {
    pub path: String,
    pub video_url: String,
    pub download_time: f32,
}
