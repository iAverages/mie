use std::{
    path::Path,
    process::{Command, Stdio},
};

use crate::types::Video;

pub async fn crop_video(video: Video, crop: &str) -> Result<String, anyhow::Error> {
    let path = video.path.split('.');

    let extension = path.clone().last().unwrap();
    let cropped_path = path
        .take_while(|&x| x != extension)
        .collect::<Vec<&str>>()
        .join(".")
        + "_cropped."
        + extension;

    let output = Command::new("ffmpeg")
        .args(["-i", video.path.as_str(), "-vf", &format!("crop={}", crop)])
        .output()
        .map_err(|_| anyhow::anyhow!("Failed to crop video"))?;

    if output.status.success() && Path::exists(Path::new(&cropped_path)) {
        Ok(cropped_path)
    } else {
        Err(anyhow::anyhow!("Failed to crop video"))
    }
}

pub fn get_crop_value(video: Video) -> Result<String, anyhow::Error> {
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
