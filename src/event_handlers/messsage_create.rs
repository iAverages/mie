use std::sync::Arc;
use tokio::time::Instant;
use twilight_model::channel::message::embed::EmbedField;
use twilight_model::gateway::payload::incoming::MessageCreate;

use crate::embed::MieEmbed;
use crate::upload::{self, upload_files};
use crate::video::download_video;
use crate::AppContext;
use url::Url;

// Wrapper function that handles errors for this event handler
pub async fn handle_message_create(ctx: Arc<AppContext>, event: MessageCreate) {
    // TODO: Handle errors better
    if let Err(err) = handle_message_create_inner(ctx, event).await {
        tracing::error!("failed to handle message event {:?}", err);
    }
}

const DEBUG: bool = cfg!(debug_assertions);

async fn handle_message_create_inner(
    ctx: Arc<AppContext>,
    event: MessageCreate,
) -> anyhow::Result<()> {
    for word in event.content.split_whitespace() {
        let is_http = word.starts_with("https://") || word.starts_with("http://");
        let is_cdn = !DEBUG && word.starts_with(&ctx.config.cdn_url);
        // Ignore if word is not a potential link or the link
        // is from the cdn url we use
        if !is_http || is_cdn {
            tracing::trace!(is_http = is_http, is_cdn = is_cdn, "ignore word {}", word);
            continue;
        }

        let video_url = Url::parse(word)?;
        let mut embed = MieEmbed::new(ctx.clone(), event.channel_id);

        // Let user know we are downloading their URL
        // also ensures we have permissions to send messages in this channel
        embed
            .title("Downloading".to_string())
            .send_or_update()
            .await?;

        let downloaded_video = download_video(&video_url.to_string()).await?;

        embed
            .title("Video Downloading, uploading original...".to_string())
            .add_field(EmbedField {
                name: "Download".to_string(),
                value: format!("{}ms", downloaded_video.download_time),
                inline: true,
            })
            .add_field(EmbedField {
                name: "Upload".to_string(),
                value: "Processing".to_string(),
                inline: true,
            })
            // .add_field(EmbedField {
            //     name: "Crop".to_string(),
            //     value: "Pending".to_string(),
            //     inline: true,
            // })
            .send_or_update()
            .await?;

        let files = vec![upload::UploadFile {
            path: downloaded_video.path.clone(),
        }];

        let bucket = Arc::new(ctx.config.b2_bucket_id.clone()).as_str().into();

        let upload_start = Instant::now();

        tracing::info!(word, "uploading start");

        let uploaded_files = upload_files(
            ctx.b2.clone(),
            bucket,
            files,
            Some(move |_path: &str, uploaded, total, percentage, bps, eta| {
                tracing::trace!(uploaded, total, percentage, bps, eta, "uploading")
                // let write = (uploaded, total, percentage, bps);
                // set_last_update_data.send(write).ok();
            }),
        )
        .await;

        let upload_time = upload_start.elapsed().as_millis();

        if let Err(err) = uploaded_files {
            tracing::error!("failed to upload files: {:?}", err);

            embed
                .title("failed to upload video".to_string())
                .update_field(
                    1,
                    EmbedField {
                        name: "Upload".to_string(),
                        value: "Error".to_string(),
                        inline: true,
                    },
                )
                .update_field(
                    2,
                    EmbedField {
                        name: "Crop".to_string(),
                        value: "Cancelled".to_string(),
                        inline: true,
                    },
                )
                .send_or_update()
                .await?;
            return Ok(());
        }

        tracing::info!(word, "uploading complete in {}ms", upload_time);

        embed
            .title(format!(
                "Download: https://cdn.avrg.dev/{}/{}.mp4",
                ctx.config.b2_bucket_path_prefix, downloaded_video.downloaded_file_name
            ))
            .update_field(
                1,
                EmbedField {
                    name: "Upload".to_string(),
                    value: format!("{}ms", upload_time),
                    inline: true,
                },
            )
            .send_or_update()
            .await?;
    }

    Ok(())
}
