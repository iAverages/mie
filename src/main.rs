// #![feature(async_closure)]
mod upload;

use std::path::PathBuf;
use std::{env, sync::Arc};

use backblaze_b2_client::structs::B2Client;
use dotenv::dotenv;
use rand::{distributions::Alphanumeric, Rng};
use serenity::async_trait;
use serenity::model::channel::Message;
use serenity::model::gateway::Ready;
use serenity::prelude::*;
use tokio::task;
use upload::upload_files;
use url::Url;
use ytd_rs::{Arg, YoutubeDL};

struct Uploader;
struct Handler;

impl TypeMapKey for Uploader {
    type Value = Arc<B2Client>;
}

#[async_trait]
impl EventHandler for Handler {
    async fn message(&self, context: Context, msg: Message) {
        if msg.author.bot {
            return;
        }

        let content = msg.content.split_whitespace();

        for word in content {
            if !(word.starts_with("https://") || word.starts_with("http://")) {
                continue;
            }
            let context = context.clone();
            let word = word.clone().to_string();
            let url = match Url::parse(&word) {
                Ok(url) => url,
                Err(_) => continue,
            };

            task::spawn(async move {
                let message_result = msg
                    .channel_id
                    .send_message(&context.http, |m| {
                        m.embed(|e| e.title("Downloading").color(11762810))
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

                let file_name = "/tmp/".to_string() + &download_name.to_string() + ".mp4";

                let args = vec![
                    Arg::new_with_arg(
                        "-f",
                        "bestvideo[ext=mp4]+bestaudio[ext=m4a]/best[ext=mp4]/best",
                    ),
                    Arg::new_with_arg("-o", &file_name),
                ];

                let path = PathBuf::from("./");
                let downloaded_file = PathBuf::from(file_name);
                let ytd = YoutubeDL::new(&path, args, &url.as_str()).unwrap();
                ytd.download().unwrap();

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

                let client = context.data.read().await;
                let b2_client = client
                    .get::<Uploader>()
                    .expect("Expected Uploader in TypeMap.")
                    .clone();

                let bucket_str = env::var("B2_BUCKET_ID").expect("No B2 bucket ID provided");
                let bucket_id = Arc::new(bucket_str).as_str().into();

                println!("Uploading to bucket: {}", bucket_id);
                println!("Uploading file: {}", downloaded_file.display());

                upload_files(
                    b2_client,
                    bucket_id,
                    vec![upload::UploadFile {
                        path: downloaded_file.into_os_string().into_string().unwrap(),
                        extra_info: None,
                    }],
                    Some(|path: &str, uploaded, total, percentage, bps, eta| {
                        println!(
                            "\rUploading {} ({}/{}) {:.2}% @ {}/s ETA: {}",
                            path, uploaded, total, percentage, bps, eta,
                        );
                    }),
                )
                .await
                .unwrap();

                let url = env::var("HOST_URL").expect("No B2 URL provided");
                let prefix = env::var("B2_BUCKET_PATH_PREFIX").expect("No B2 prefix provided");
                let url = url + &prefix + "/" + &download_name + ".mp4";

                update_message
                    .edit(&context.http, |m| {
                        m.embed(|e| e.title("Uploaded to B2").description(&url).color(11762810))
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
    println!("using b2 key id: {}", b2_key_id);
    println!("using b2 application key: {}", b2_application_key);

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
