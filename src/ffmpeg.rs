use std::{
    path::PathBuf,
    process::{Command, Stdio},
};

use serenity::{client::Context, model::channel::Message, utils::Colour};
use ytd_rs::{Arg, YoutubeDL};

use crate::types::DownloadedVideo;

const EMBED_COLOR: Colour = Colour::new(11762810);

pub fn crop_video(file_name: &String, cropped_file_name: &String) -> bool {
    let crop_detech = match Command::new("ffmpeg")
        .args([
            "-i",
            file_name,
            "-t",
            "1",
            "-vf",
            "cropdetect",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::piped())
        .output()
    {
        Ok(crop_detech) => crop_detech,
        Err(why) => {
            println!("Error cropping video: {:?}", why);
            return false;
        }
    };

    let crop_detech = String::from_utf8_lossy(&crop_detech.stderr);

    let crop_w_h = match crop_detech
        .lines()
        .filter(|line| line.contains("crop="))
        .map(|f| {
            let crop = f
                .split("crop=")
                .last()
                .unwrap()
                .split(':')
                .map(|f| f.parse::<i32>().unwrap());

            let crop_w_h = crop.clone().take(2).reduce(|a, b| (a * b)).unwrap();
            (crop_w_h, crop)
        })
        .max_by(|a, b| a.0.cmp(&b.0))
    {
        Some((_, crop)) => crop.collect::<Vec<i32>>(),
        None => {
            println!("Could not find crop");
            return false;
        }
    };

    let offset = 40;
    let (crop_w, crop_x) = get_crop_size(crop_w_h[0], crop_w_h[2], offset);
    let (crop_h, crop_y) = get_crop_size(crop_w_h[1], crop_w_h[3], offset);

    let crop = format!("{}:{}:{}:{}", crop_w, crop_h, crop_x, crop_y);

    match Command::new("ffmpeg")
        .args([
            "-i",
            file_name,
            "-vf",
            &format!("crop={}", crop),
            cropped_file_name,
        ])
        .output()
    {
        Ok(out) => {
            if !out.status.success() {
                println!("Error cropping video: {:?}", out);
                return false;
            }

            true
        }
        Err(why) => {
            println!("Error cropping video: {:?}", why);
            false
        }
    }
}
// pub async fn crop_video(video: Video, crop: &str) -> Result<String, anyhow::Error> {
//     let path = video.path.split('.');

//     let extension = path.clone().last().unwrap();
//     let cropped_path = path
//         .take_while(|&x| x != extension)
//         .collect::<Vec<&str>>()
//         .join(".")
//         + "_cropped."
//         + extension;

//     let output = Command::new("ffmpeg")
//         .args(["-i", video.path.as_str(), "-vf", &format!("crop={}", crop)])
//         .output()
//         .map_err(|_| anyhow::anyhow!("Failed to crop video"))?;

//     if output.status.success() && Path::exists(Path::new(&cropped_path)) {
//         Ok(cropped_path)
//     } else {
//         Err(anyhow::anyhow!("Failed to crop video"))
//     }
// }

pub fn get_crop_value(video: DownloadedVideo) -> Result<String, anyhow::Error> {
    let output = Command::new("ffmpeg")
        .args([
            "-i",
            video.path.as_str(),
            "-t",
            "1",
            "-vf",
            "cropdetect",
            "-f",
            "null",
            "-",
        ])
        .stdout(Stdio::piped())
        .output()
        .map_err(|_| anyhow::anyhow!("Failed to find crop value for video".to_string()))?;

    if !output.status.success() {
        return Err(anyhow::anyhow!("Failed to find crop value for video"));
    }

    let output = String::from_utf8_lossy(&output.stderr);

    let crop_w_h = output
        .lines()
        .filter(|line| line.contains("crop="))
        .map(|f| {
            let crop = f
                .split("crop=")
                .last()
                .unwrap()
                .split(':')
                .map(|f| f.parse::<i32>().unwrap());

            let crop_w_h = crop.clone().take(2).reduce(|a, b| (a * b)).unwrap();
            (crop_w_h, crop)
        })
        .max_by(|a, b| a.0.cmp(&b.0))
        .ok_or(anyhow::anyhow!("Failed to find crop value for video"))?;

    let crop_w_h = crop_w_h.1.collect::<Vec<i32>>();

    let offset = 40;
    let (crop_w, crop_x) = get_crop_size(crop_w_h[0], crop_w_h[2], offset);
    let (crop_h, crop_y) = get_crop_size(crop_w_h[1], crop_w_h[3], offset);

    let crop = format!("{}:{}:{}:{}", crop_w, crop_h, crop_x, crop_y);

    Ok(crop)
}

pub fn generate_thumbnail(video_path: &str) -> String {
    let output = Command::new("ffmpeg")
        .arg("-i")
        .arg(video_path)
        .arg("-ss")
        .arg("00:00:01.000")
        .arg("-vframes")
        .arg("1")
        .arg("-q:v")
        .arg("2")
        .arg("-f")
        .arg("image2")
        .arg("-")
        .output()
        .expect("failed to execute process");

    // let mut file = File::create("test.jpg").unwrap();
    // file.write_all(&output.stdout).unwrap();

    "test.jpg".to_string()
}

fn get_crop_size(dim: i32, pos: i32, unit: i32) -> (i32, i32) {
    if pos == 0 {
        return (dim, pos);
    }

    let mut changed_by = unit;
    let mut new_pos = pos - changed_by;

    if new_pos < 0 {
        new_pos = 0;
        changed_by = new_pos.abs();
    }

    let new_dim = dim + changed_by;

    (new_dim, new_pos)
}

pub async fn download_video(
    download_name: &String,
    video_url: &String,
    process_start: &std::time::Instant,
    update_message: &mut Message,
    context: &Context,
) -> Result<DownloadedVideo, anyhow::Error> {
    let file_name = "/tmp/mie/".to_string() + &download_name.to_string() + ".mp4";

    let args = vec![
        Arg::new_with_arg(
            "-f",
            "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
        ),
        Arg::new_with_arg("-o", &file_name),
    ];

    let path = PathBuf::from("/tmp/mie");
    let downloaded_file = PathBuf::from(&file_name);
    let ytd = match YoutubeDL::new(&path, args, video_url.as_str()) {
        Ok(ytd) => ytd,
        Err(why) => {
            println!("Error creating YoutubeDL: {:?}", why);
            update_message
                .edit(&context.http, |m| {
                    m.embed(|e| {
                        e.title("Error downloading video YTDL_INIT_ERROR")
                            .color(EMBED_COLOR)
                    })
                })
                .await
                .unwrap();
            return Err(anyhow::anyhow!("Error creating YoutubeDL instance"));
        }
    };
    match ytd.download() {
        Ok(_) => {}
        Err(why) => {
            println!("Error downloading video: {:?}", why);
            update_message
                .edit(&context.http, |m| {
                    m.embed(|e| {
                        e.title("Error downloading video YTDL_DOWNLOAD_ERROR")
                            .color(EMBED_COLOR)
                    })
                })
                .await
                .unwrap();
            return Err(anyhow::anyhow!("Error downloading video"));
        }
    };

    let download_complete = process_start.elapsed().as_secs_f32();

    Ok(DownloadedVideo {
        path: downloaded_file.to_str().unwrap().to_string(),
        video_url: video_url.to_string(),
        download_time: download_complete,
    })
}
