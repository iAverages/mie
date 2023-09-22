// #![feature(async_closure)]
mod upload;

use pretty_bytes::converter::convert;
use std::f32::consts::E;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use std::{env, sync::Arc};

use backblaze_b2_client::structs::B2Client;
use dotenv::dotenv;
use futures::FutureExt;
use pretty_duration::{pretty_duration, PrettyDurationOptions, PrettyDurationOutputFormat};
use rand::{distributions::Alphanumeric, Rng};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use serenity::utils::Colour;
use tokio::task;
use tokio_util::sync::CancellationToken;
use upload::upload_files;
use url::Url;
use ytd_rs::{Arg, YoutubeDL};

struct Uploader;
struct Handler;

impl TypeMapKey for Uploader {
    type Value = Arc<B2Client>;
}

const EMBED_COLOR: Colour = Colour::new(11762810);

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let content = msg.content.split_whitespace();
        let host_url = env::var("HOST_URL").expect("no host URL provided");
        let path_prefix = env::var("B2_BUCKET_PATH_PREFIX").expect("no path prefix provided");
        let cdn_url = host_url + &path_prefix;

        for word in content {
            if !(word.starts_with("https://") || word.starts_with("http://"))
            // && !word.starts_with(&cdn_url))
            {
                continue;
            }

            let context = context.clone();
            let word = word.clone().to_string();
            let url = match Url::parse(&word) {
                Ok(url) => url,
                Err(_) => continue,
            };

            task::spawn(async move {
                let process_start = Instant::now();
                let message_result = msg
                    .channel_id
                    .send_message(&context.http, |m| {
                        m.embed(|e| e.title("Downloading").color(EMBED_COLOR))
                    })
                    .await;

                let mut update_message = match message_result {
                    Ok(m) => m,
                    Err(why) => {
                        println!("Error sending message: {:?}", why);
                        return;
                    }
                };

                let download_name: String = rand::thread_rng()
                    .sample_iter(&Alphanumeric)
                    .take(7)
                    .map(char::from)
                    .collect();

                let file_name = "/tmp/mie/".to_string() + &download_name.to_string() + ".mp4";
                let file_name_b = file_name.clone();

                let args = vec![
                    Arg::new_with_arg(
                        "-f",
                        "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                    ),
                    Arg::new_with_arg("-o", &file_name),
                ];

                let path = PathBuf::from("/tmp/mie");
                let downloaded_file = PathBuf::from(&file_name);
                let ytd = YoutubeDL::new(&path, args, &url.as_str()).unwrap();
                ytd.download().unwrap();

                let download_complete = process_start.elapsed().as_secs_f32();
                println!("Download complete in {} seconds", download_complete);

                let crop_start = Instant::now();

                let cropped_file_name =
                    "/tmp/mie/".to_string() + &download_name.to_string() + "_cropped.mp4";

                // let crop_detech = Command::new("ffmpeg")
                //     .args(&[
                //         "-i",
                //         &file_name,
                //         "-t",
                //         "1",
                //         "-vf",
                //         "cropdetect",
                //         "-f",
                //         "null",
                //         "-",
                //     ])
                //     .stdout(Stdio::piped())
                //     .output()
                //     .expect("failed to execute process");

                // let crop_detech = String::from_utf8_lossy(&crop_detech.stderr);
                // let crop = crop_detech
                //     .lines()
                //     .filter(|line| line.contains("crop="))
                //     .last()
                //     .unwrap()
                //     .split("crop=")
                //     .last()
                //     .unwrap()
                //     .split(")")
                //     .next()
                //     .unwrap();

                // Command::new("ffmpeg")
                //     .args(&[
                //         "-i",
                //         &file_name_b,
                //         "-vf",
                //         &format!("crop={}", crop),
                //         &cropped_file_name,
                //     ])
                //     .output()
                //     .expect("failed to execute process");

                // let cropped_meta = fs::metadata(&cropped_file_name).unwrap();
                // let cropped_size = cropped_meta.len();
                // let was_cropped = cropped_size > 0;
                let was_cropped = false;

                if was_cropped {
                    // println!("Cropped to {}", crop);
                }

                let crop_complete = crop_start.elapsed().as_secs_f32();

                update_message
                    .edit(&context.http, |m| {
                        m.embed(|e| {
                            e.title("Uploading to B2")
                                .description(&download_name)
                                .color(11762810)
                        })
                    })
                    .await
                    .unwrap();

                let upload_start = Instant::now();

                let client = context.data.read().await;
                let b2_client = client
                    .get::<Uploader>()
                    .expect("Expected Uploader in TypeMap.")
                    .clone();

                let bucket_str = env::var("B2_BUCKET_ID").expect("No B2 bucket ID provided");
                let bucket_id = Arc::new(bucket_str).as_str().into();

                println!("Uploading file: {}", downloaded_file.display());

                let mut files_to_upload = Vec::<upload::UploadFile>::new();

                files_to_upload.push(upload::UploadFile {
                    path: downloaded_file.into_os_string().into_string().unwrap(),
                    extra_info: None,
                });

                if was_cropped {
                    files_to_upload.push(upload::UploadFile {
                        path: cropped_file_name,
                        extra_info: None,
                    });
                }

                let (set_last_update_data, mut last_update_data) =
                    lockfree::channel::mpsc::create::<(u64, u64, f32, u64)>();

                let update_status_message =
                    Arc::new(futures_locks::RwLock::new(update_message.clone()));
                let download_name_cb = download_name.clone();

                let token = CancellationToken::new();
                let cloned_token = token.clone();

                let http = context.http.clone();

                tokio::spawn(async move {
                    loop {
                        println!("Checking upload status");
                        tokio::time::sleep(Duration::from_secs(5)).await;
                        if cloned_token.is_cancelled() {
                            println!("Upload complete or cancelled");
                            break;
                        }

                        let mut last_data = last_update_data.recv().unwrap();

                        let data = loop {
                            match last_update_data.recv() {
                                Ok(data) => {
                                    last_data = data;
                                    continue;
                                }
                                Err(_) => {
                                    break last_data;
                                }
                            }
                        };

                        let (uploaded, total, percentage, bps) = (data.0, data.1, data.2, data.3);

                        println!(
                            "TASK: Upload progress: {}/{} ({}%) at {}",
                            uploaded,
                            total,
                            percentage * 100.0,
                            bps
                        );

                        let mut message = update_status_message
                            .write()
                            .now_or_never()
                            .unwrap()
                            .clone();

                        message
                            .edit(&http, |m| {
                                m.embed(|e| {
                                    e.title("Uploading to B2")
                                        .description(&download_name_cb)
                                        .color(11762810)
                                        .fields([
                                            (
                                                "Progress",
                                                &format!(
                                                    "{}/{}",
                                                    convert(uploaded as f64),
                                                    convert(total as f64)
                                                ),
                                                true,
                                            ),
                                            (
                                                "Percentage",
                                                &format!("{}%", percentage * 100.0),
                                                true,
                                            ),
                                            ("Speed", &format!("{}", convert(bps as f64)), true),
                                        ])
                                })
                            })
                            .await
                            .expect("Could not edit message");
                    }
                });

                upload_files(
                    b2_client,
                    bucket_id,
                    files_to_upload,
                    Some(move |_path: &str, uploaded, total, percentage, bps, _eta| {
                        let write = (uploaded, total, percentage, bps);
                        println!(
                            "ONCHUNK: Upload progress: {}/{} ({}%) at {}",
                            uploaded,
                            total,
                            percentage * 100.0,
                            bps
                        );
                        set_last_update_data.send(write).unwrap();
                    }),
                )
                .await
                .unwrap();

                token.cancel();

                print!("Upload complete");
                let upload_complete = upload_start.elapsed().as_secs_f32();

                let url = env::var("HOST_URL").expect("No B2 URL provided");
                let prefix = env::var("B2_BUCKET_PATH_PREFIX").expect("No B2 prefix provided");
                let og_url = url.clone() + &prefix + "/" + &download_name + ".mp4";
                let cropped_url = url + &prefix + "/" + &download_name + "_cropped.mp4";

                let format_options = Some(PrettyDurationOptions {
                    output_format: Some(PrettyDurationOutputFormat::Compact),
                    plural_labels: None,
                    singular_labels: None,
                });

                let pretty_download = pretty_duration(
                    &Duration::from_secs_f64(download_complete as f64),
                    format_options.clone(),
                );
                let pretty_crop = pretty_duration(
                    &Duration::from_secs_f64(crop_complete as f64),
                    format_options.clone(),
                );
                let pretty_upload = pretty_duration(
                    &Duration::from_secs_f64(upload_complete as f64),
                    format_options,
                );

                let mut fields = Vec::<(&str, String, bool)>::new();
                fields.push(("Download", format!("{}", pretty_download), true));
                if was_cropped {
                    fields.push(("Crop", format!("{}", pretty_crop), true));
                }
                fields.push(("Upload", format!("{}", pretty_upload), true));

                let desc = "Original: ".to_owned() + &og_url;
                let desc = match was_cropped {
                    true => desc + "\nCropped: " + &cropped_url,
                    false => desc,
                };

                println!("done");
                update_message
                    .edit(&context.http, |m| {
                        m.embed(|e| {
                            e.title("Uploaded to B2")
                                .description(desc)
                                .color(11762810)
                                .fields(fields)
                        })
                    })
                    .await
                    .unwrap();
            });
        }
    }

    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    let discord_token = env::var("DISCORD_TOKEN").expect("No Discord token provided");
    let b2_key_id = env::var("B2_APPLICATION_KEY_ID").expect("No B2 key ID provided");
    let b2_application_key =
        env::var("B2_APPLICATION_KEY").expect("No B2 application key provided");

    env::var("B2_BUCKET_NAME").expect("B2_BUCKET_NAME not set");
    env::var("B2_APPLICATION_KEY").expect("B2_APPLICATION_KEY not set");
    env::var("B2_URL").expect("B2_URL not set");

    println!("Starting up...");

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&discord_token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    let b2_client = B2Client::new(&b2_key_id, &b2_application_key)
        .await
        .unwrap();

    {
        let mut data = client.data.write().await;
        data.insert::<Uploader>(Arc::new(b2_client));
    }

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}