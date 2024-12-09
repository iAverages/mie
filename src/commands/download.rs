use std::sync::Arc;
use std::time::Instant;

use twilight_model::channel::message::embed::EmbedField;
use url::Url;
use vesper::prelude::*;

use crate::embed::MieEmbed;
use crate::upload::{self, upload_files};
use crate::video::download_video;
use crate::AppContext;

#[command(chat)]
#[description = "Download a video"]
pub async fn download(
    ctx: &mut SlashContext<Arc<AppContext>>,
    #[description = "URL To Download"] url: String,
    #[description = "Extra text to inlude in message"] content: Option<String>,
) -> DefaultCommandResult {
    ctx.defer(true).await?;
    let is_http = url.starts_with("https://") || url.starts_with("http://");

    // Ignore if word is not a potential link
    if !is_http {
        ctx.interaction_client
            .update_response(&ctx.interaction.token)
            .content(Some("give me a link you stupid fuck"))?
            .await?;

        return Ok(());
    }

    let video_url = Url::parse(&url)?;
    // TODO: Fix unwarp
    let channel = ctx.interaction.channel.clone().unwrap();
    let channel_id = channel.id;
    let mut embed = MieEmbed::new(ctx.data.clone(), channel_id);

    // Let user know we are downloading their URL
    // also ensures we have permissions to send messages in this channel
    ctx.interaction_client
        .update_response(&ctx.interaction.token)
        .embeds(Some(&[embed.title("Downloading".to_string()).build()]))?
        .await?;

    let downloaded_video = download_video(&video_url.to_string()).await?;

    ctx.interaction_client
        .update_response(&ctx.interaction.token)
        .embeds(Some(&[embed
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
            .build()]))?
        .await?;

    let files = vec![upload::UploadFile {
        path: downloaded_video.path.clone(),
        extra_info: None,
    }];

    let bucket = Arc::new(ctx.data.config.b2_bucket_id.clone())
        .as_str()
        .into();

    let upload_start = Instant::now();

    tracing::info!(url, "uploading start");

    let uploaded_files = upload_files(
        ctx.data.b2.clone(),
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
        ctx.interaction_client
            .update_response(&ctx.interaction.token)
            .embeds(Some(&[embed
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
                .build()]))?
            .await?;
        return Ok(());
    }

    tracing::info!(url, "uploading complete in {}ms", upload_time);
    ctx.interaction_client
        .update_response(&ctx.interaction.token)
        .embeds(Some(&[embed
            .title(format!(
                "Download: https://cdn.avrg.dev/{}/{}.mp4",
                ctx.data.config.b2_bucket_path_prefix, downloaded_video.downloaded_file_name
            ))
            .update_field(
                1,
                EmbedField {
                    name: "Upload".to_string(),
                    value: format!("{}ms", upload_time),
                    inline: true,
                },
            )
            .build()]))?
        .await?;
    ctx.interaction_client
        .create_followup(&ctx.interaction.token)
        .content(
            format!(
                "{} https://cdn.avrg.dev/{}/{}.mp4",
                content.unwrap_or("".to_string()),
                ctx.data.config.b2_bucket_path_prefix,
                downloaded_video.downloaded_file_name
            )
            .as_str(),
        )?
        .await?;

    tracing::info!("donme?");

    Ok(())
}
