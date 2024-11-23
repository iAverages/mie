use std::path::PathBuf;
use std::time::Instant;

use ytd_rs::{Arg, YoutubeDL};

pub struct DownloadedVideo {
    pub og_url: String,
    pub path: String,
    pub download_time: u128,
}

pub async fn download_video(
    download_name: &String,
    video_url: &String,
) -> Result<DownloadedVideo, anyhow::Error> {
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

    Ok(DownloadedVideo {
        path: file_name,
        og_url: video_url.to_string(),
        download_time: process_start.elapsed().as_millis(),
    })
}
