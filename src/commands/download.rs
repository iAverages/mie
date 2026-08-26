use std::fs;
use std::sync::Arc;
use std::time::Instant;

use twilight_model::application::interaction::{
    application_command::{CommandData, CommandOptionValue},
    Interaction,
};
use twilight_model::channel::message::embed::EmbedField;
use twilight_model::channel::message::MessageFlags;
use twilight_model::http::interaction::{
    InteractionResponse, InteractionResponseData, InteractionResponseType,
};
use url::Url;

use crate::embed::MieEmbed;
use crate::errors::MieError;
use crate::upload::{self, upload_files};
use crate::video::download_video;
use crate::AppContext;

pub async fn download(ctx: Arc<AppContext>, interaction: Interaction, data: CommandData) {
    let url = string_option(&data, "url");
    let content = string_option(&data, "content");
    let Some(url) = url else {
        tracing::error!("download command missing url option");
        return;
    };

    match download_inner(&ctx, &interaction, url, content).await {
        Ok(()) => {}
        Err(err) => {
            let channel = interaction.channel.as_ref().unwrap();
            let channel_id = channel.id;
            let mut embed = MieEmbed::new(ctx.clone(), channel_id);
            let error_embed;

            if let Some(mie_error) = err.downcast_ref::<MieError>() {
                match mie_error {
                    MieError::VideoDownloadFailed(video) => {
                        error_embed =
                            embed.title(format!("failed to download video: {}", video.og_url));
                    }
                    MieError::YtDlError(_) => {
                        error_embed = embed.title("ytdlp errored".to_string());
                    }
                }
            } else {
                tracing::error!("unhandled error: {}", err.to_string());
                error_embed = embed.title("An error occured while downloading video".to_string());
            }

            ctx.http
                .interaction(interaction.application_id)
                .update_response(&interaction.token)
                .embeds(Some(&[error_embed.build()]))
                .await
                .ok();
        }
    }
}

fn string_option(data: &CommandData, name: &str) -> Option<String> {
    data.options.iter().find_map(|option| match &option.value {
        CommandOptionValue::String(value) if option.name == name => Some(value.clone()),
        _ => None,
    })
}

use std::error::Error;
async fn download_inner(
    ctx: &Arc<AppContext>,
    interaction: &Interaction,
    url: String,
    content: Option<String>,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    let interaction_client = ctx.http.interaction(interaction.application_id);
    interaction_client
        .create_response(
            interaction.id,
            &interaction.token,
            &InteractionResponse {
                kind: InteractionResponseType::DeferredChannelMessageWithSource,
                data: Some(InteractionResponseData {
                    flags: Some(MessageFlags::EPHEMERAL),
                    ..InteractionResponseData::default()
                }),
            },
        )
        .await?;
    let is_http = url.starts_with("https://") || url.starts_with("http://");

    // Ignore if word is not a potential link
    if !is_http {
        interaction_client
            .update_response(&interaction.token)
            .content(Some("give me a link you stupid fuck"))
            .await?;

        return Ok(());
    }

    let video_url = Url::parse(&url)?;
    // TODO: Fix unwarp
    let channel = interaction.channel.as_ref().unwrap();
    let channel_id = channel.id;
    let mut embed = MieEmbed::new(ctx.clone(), channel_id);

    // Let user know we are downloading their URL
    // also ensures we have permissions to send messages in this channel
    interaction_client
        .update_response(&interaction.token)
        .embeds(Some(&[embed.title("Downloading".to_string()).build()]))
        .await?;

    let downloaded_video = download_video(&video_url.to_string()).await?;

    interaction_client
        .update_response(&interaction.token)
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
            .build()]))
        .await?;

    let files = vec![upload::UploadFile {
        path: downloaded_video.path.clone(),
    }];

    let bucket = Arc::new(ctx.config.b2_bucket_id.clone()).as_str().into();

    let upload_start = Instant::now();

    tracing::info!(url, "uploading start");

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

    let _ = fs::remove_file(downloaded_video.path);

    let upload_time = upload_start.elapsed().as_millis();

    if let Err(err) = uploaded_files {
        tracing::error!("failed to upload files: {:?}", err);
        interaction_client
            .update_response(&interaction.token)
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
                .build()]))
            .await?;
        return Ok(());
    }

    tracing::info!(url, "uploading complete in {}ms", upload_time);
    interaction_client
        .update_response(&interaction.token)
        .embeds(Some(&[embed
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
            .build()]))
        .await?;
    interaction_client
        .create_followup(&interaction.token)
        .content(&format!(
            "{} https://cdn.avrg.dev/{}/{}.mp4",
            content.unwrap_or_default(),
            ctx.config.b2_bucket_path_prefix,
            downloaded_video.downloaded_file_name
        ))
        .await?;

    tracing::info!("donme?");

    Ok(())
}
