// #![feature(async_closure)]
mod database;
mod embed;
mod ffmpeg;
mod types;
mod upload;

use ffmpeg::crop_video;
use pretty_bytes::converter::convert;
use std::process::Command;
use std::time::{Duration, Instant};
use std::{env, sync::Arc};
use std::{fs, process};
use types::AddMedia;

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

impl TypeMapKey for database::DB {
    type Value = database::DB;
}

struct Uploader;

impl TypeMapKey for Uploader {
    type Value = Arc<Mutex<B2Client>>;
}

const MAX_CROP_SECONDS: i32 = 60;
const TIME_BETWEEN_B2_AUTH: u64 = 79200; // 22 hours
const EMBED_COLOR: Colour = Colour::new(11762810);

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let content = msg.content.split_whitespace();
        let host_url = env::var("HOST_URL").expect("no host URL provided");
        let path_prefix = env::var("B2_BUCKET_PATH_PREFIX").expect("no path prefix provided");
        let debug = env::var("DEBUG").unwrap_or_else(|_| "false".to_string()) == "true";
        let cdn_url = host_url + &path_prefix;

        for word in content {
            let is_http = word.starts_with("https://") || word.starts_with("http://");
            let is_cdn = !debug && word.starts_with(&cdn_url);
            if !is_http || is_cdn {
                continue;
            }

            let context = context.clone();
            let video_url = match Url::parse(word) {
                Ok(url) => url,
                Err(_) => continue,
            };

            task::spawn(async move {
                let process_start = Instant::now();

                // Ensure we have permissions to send messages in this channel
                let message_result = msg
                    .channel_id
                    .send_message(&context.http, |m| {
                        m.embed(|e| e.title("Downloading").color(EMBED_COLOR))
                    })
                    .await;

                // If we can't send a messages, exist early
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

                let downloaded_video = match ffmpeg::download_video(
                    &download_name,
                    &video_url.to_string(),
                    &process_start,
                    &mut update_message,
                    &context,
                )
                .await
                {
                    Ok(video) => video,
                    Err(why) => {
                        println!("Error downloading video: {:?}", why);
                        return;
                    }
                };

                println!(
                    "Download complete in {} seconds",
                    downloaded_video.download_time
                );
                println!("Checking video duration");

                let crop_start = Instant::now();

                let cropped_file_name =
                    "/tmp/mie/".to_string() + &download_name.to_string() + "_cropped.mp4";

                let duration = match Command::new("ffprobe")
                    .args([
                        "-v",
                        "error",
                        "-show_entries",
                        "format=duration",
                        "-of",
                        "default=noprint_wrappers=1:nokey=1",
                        &downloaded_video.path,
                    ])
                    .output()
                {
                    Ok(duration) => String::from_utf8_lossy(&duration.stdout)
                        .split('.')
                        .next()
                        .unwrap()
                        .to_string()
                        .parse::<i32>()
                        .unwrap_or(-1),
                    Err(err) => {
                        println!("Error getting video duration: {:?}", err);
                        -1
                    }
                };

                println!("Video duration: {}", duration);

                let mut was_cropped = false;
                if MAX_CROP_SECONDS > duration && duration > 0 {
                    println!("Video is {} seconds, cropping", duration);
                    was_cropped = crop_video(&downloaded_video.path, &cropped_file_name);
                }

                let crop_complete = crop_start.elapsed().as_secs_f32();

                update_message
                    .edit(&context.http, |m| {
                        m.embed(|e| e.title("Uploading to B2").color(11762810))
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

                println!("Uploading file: {}", downloaded_video.path);

                let mut files_to_upload = Vec::<upload::UploadFile>::new();

                files_to_upload.push(upload::UploadFile {
                    path: downloaded_video.path.clone(),
                    extra_info: None,
                });

                if was_cropped {
                    files_to_upload.push(upload::UploadFile {
                        path: cropped_file_name.clone(),
                        extra_info: None,
                    });
                }

                let (set_last_update_data, mut last_update_data) =
                    lockfree::channel::mpsc::create::<(u64, u64, f32, u64)>();

                let update_status_message =
                    Arc::new(futures_locks::RwLock::new(update_message.clone()));

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

                        let mut last_data = match last_update_data.recv() {
                            Ok(data) => data,
                            Err(_) => {
                                continue;
                            }
                        };

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
                                    e.title("Uploading to B2").color(11762810).fields([
                                        (
                                            "Progress",
                                            &format!(
                                                "{}/{}",
                                                convert(uploaded as f64),
                                                convert(total as f64)
                                            ),
                                            true,
                                        ),
                                        ("Percentage", &format!("{}%", percentage * 100.0), true),
                                        ("Speed", &convert(bps as f64).to_string(), true),
                                    ])
                                })
                            })
                            .await
                            .expect("Could not edit message");
                    }
                });

                let uploaded_files = upload_files(
                    b2_client,
                    bucket_id,
                    files_to_upload,
                    Some(move |_path: &str, uploaded, total, percentage, bps, _eta| {
                        let write = (uploaded, total, percentage, bps);
                        set_last_update_data.send(write).ok();
                    }),
                )
                .await;

                token.cancel();

                match uploaded_files {
                    Ok(_) => {}
                    Err(why) => {
                        println!("Error uploading file: {:?}", why);
                        update_message
                            .edit(&context.http, |m| {
                                m.embed(|e| {
                                    e.title("Error uploading video B2_UPLOAD_ERROR")
                                        .color(EMBED_COLOR)
                                })
                            })
                            .await
                            .unwrap();
                        return;
                    }
                };

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
                    &Duration::from_secs_f64(downloaded_video.download_time as f64),
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
                fields.push(("Download", pretty_download.to_string(), true));
                if was_cropped {
                    fields.push(("Crop", pretty_crop.to_string(), true));
                }
                fields.push(("Upload", pretty_upload.to_string(), true));

                let desc = "Original: ".to_owned() + &og_url;
                let desc = match was_cropped {
                    true => desc + "\nCropped: " + &cropped_url,
                    false => desc,
                };

                let file_size = fs::metadata(&downloaded_video.path).unwrap().len();

                fs::remove_file(downloaded_video.path.clone()).unwrap_or_else(|_| {
                    println!("Could not remove file: {}", &downloaded_video.path);
                });
                fs::remove_file(&cropped_file_name).unwrap_or_else(|_| {
                    println!("Could not remove file: {}", &cropped_file_name);
                });
                println!("done");

                let db = client
                    .get::<database::DB>()
                    .expect("Expected DB in TypeMap.");

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

                db.add(AddMedia {
                    file_type: "video/mp4".to_string(),
                    url: og_url.clone(),
                    actual_source: None,
                    original_source: video_url.to_string(),
                    size: file_size as i32,
                    meta: serde_json::Value::Null,
                    uploader: msg.author.id.to_string(),
                })
                .await;
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
    let database_url = env::var("DATABASE_URL").expect("No database URL provided");

    env::var("B2_BUCKET_NAME").expect("B2_BUCKET_NAME not set");
    env::var("B2_APPLICATION_KEY").expect("B2_APPLICATION_KEY not set");
    env::var("B2_URL").expect("B2_URL not set");

    println!("Starting up...");
    println!("Connecting to database");

    let db = match database::DB::new(&database_url).await {
        Ok(db) => db,
        Err(why) => {
            println!("Error creating database: {:?}", why);
            process::exit(1)
        }
    };

    let intents = GatewayIntents::GUILD_MESSAGES
        | GatewayIntents::DIRECT_MESSAGES
        | GatewayIntents::MESSAGE_CONTENT;
    let mut client = Client::builder(&discord_token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    let b2_client = Arc::new(Mutex::new(
        B2Client::new(&b2_key_id.clone(), &b2_application_key.clone())
            .await
            .unwrap(),
    ));

    {
        let mut data = client.data.write().await;
        data.insert::<Uploader>(b2_client.clone());
        data.insert::<database::DB>(db);
    }

    tokio::spawn(async move {
        println!("Starting reauth thread");
        let key_id = &b2_key_id.clone();
        let application_key = &b2_application_key.clone();
        let b2_client = &b2_client.clone();
        loop {
            tokio::time::sleep(Duration::from_secs(TIME_BETWEEN_B2_AUTH)).await;
            println!("Reauthorizing B2 account");
            b2_client
                .lock()
                .await
                .authorize_account(key_id, application_key)
                .await
                .expect("Could not authorize B2 account");

            println!("Done! Waiting {} seconds", TIME_BETWEEN_B2_AUTH);
        }
    });

    if let Err(why) = client.start().await {
        println!("Client error: {:?}", why);
    }
}
