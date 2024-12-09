use rand::distributions::Alphanumeric;
use rand::Rng;
use std::path::PathBuf;
use std::time::Instant;

use ytd_rs::{Arg, YoutubeDL};

pub struct DownloadedVideo {
    pub og_url: String,
    pub path: String,
    pub download_time: u128,
    pub downloaded_file_name: String,
}

pub async fn download_video(video_url: &String) -> Result<DownloadedVideo, anyhow::Error> {
    let download_name: String = rand::thread_rng()
        .sample_iter(&Alphanumeric)
        .take(7)
        .map(char::from)
        .collect();

    tracing::info!(video_url, download_name, "Downloading");
    let process_start = Instant::now();
    let file_name = "/tmp/mie/".to_string() + &download_name.to_string() + ".mp4";

    let args = vec![
        Arg::new_with_arg(
            "-f",
            "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
        ),
        Arg::new_with_arg("-o", &file_name),
    ];

    let path = PathBuf::from("/tmp/mie");
    let ytd = YoutubeDL::new(&path, args, video_url.as_str())?;
    ytd.download()?;

    let download_time = process_start.elapsed().as_millis();
    tracing::info!(video_url, "Downloading took {}ms", download_time);

    Ok(DownloadedVideo {
        path: file_name,
        og_url: video_url.to_string(),
        download_time,
        downloaded_file_name: download_name,
    })
}
