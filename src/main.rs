mod commands;
mod embed;
mod env;
mod errors;
mod event_handlers;
mod upload;
mod video;

use std::error::Error;
use std::sync::Arc;

use backblaze_b2_client::client::B2Client;
use tracing_subscriber::EnvFilter;
use twilight_gateway::{ConfigBuilder, Event, EventTypeFlags, Intents, Shard, ShardId, StreamExt};
use twilight_http::Client as HttpClient;
use twilight_model::application::command::CommandType;
use twilight_model::application::interaction::InteractionContextType;
use twilight_model::application::interaction::{InteractionData, InteractionType};
use twilight_model::id::marker::ApplicationMarker;
use twilight_model::id::Id;
use twilight_model::oauth::ApplicationIntegrationType;
use twilight_util::builder::command::{CommandBuilder, StringBuilder};

use self::commands::download::download;
use self::env::{create_config, load_env, Config};
use self::event_handlers::messsage_create::handle_message_create;

pub struct AppContext {
    config: Config,
    http: Arc<HttpClient>,
    b2: Arc<B2Client>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    load_env()?;
    rustls::crypto::ring::default_provider()
        .install_default()
        .unwrap();

    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = create_config();

    let shard_config = ConfigBuilder::new(
        config.discord_token.clone(),
        Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::DIRECT_MESSAGES,
    )
    .build();
    let event_types = EventTypeFlags::MESSAGE_CREATE
        | EventTypeFlags::GATEWAY_HELLO
        | EventTypeFlags::READY
        | EventTypeFlags::INTERACTION_CREATE;

    let mut shard = Shard::with_config(ShardId::ONE, shard_config);

    let b2 = Arc::new(
        B2Client::new(config.b2_key_id.clone(), config.b2_application_key.clone())
            .await
            .unwrap(),
    );

    // HTTP is separate from the gateway, so create a new client.
    let http = Arc::new(HttpClient::new(config.discord_token.clone()));

    let app_id = get_application_id(&http)
        .await
        .expect("Failed to get application id for current bot token");

    let app_context = Arc::new(AppContext {
        config: config.clone(),
        http: http.clone(),
        b2,
    });

    tracing::info!("creating download command");
    let command = CommandBuilder::new("download", "Download a video", CommandType::ChatInput)
        .contexts([
            InteractionContextType::Guild,
            InteractionContextType::BotDm,
            InteractionContextType::PrivateChannel,
        ])
        .integration_types([
            ApplicationIntegrationType::GuildInstall,
            ApplicationIntegrationType::UserInstall,
        ])
        .option(
            StringBuilder::new("url", "URL To Download")
                .required(true)
                .build(),
        )
        .option(StringBuilder::new("content", "Extra text to include in message").build())
        .build();
    http.interaction(app_id)
        .set_global_commands(&[command])
        .await?;

    loop {
        match shard.next_event(event_types).await {
            Some(Ok(item)) => {
                tokio::spawn(handle_event(Arc::clone(&app_context), item));
            }
            Some(Err(err)) => {
                tracing::warn!(source = ?err, "error receiving event");
            }
            None => break,
        }
    }

    Ok(())
}

async fn handle_event(
    ctx: Arc<AppContext>,
    event: Event,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if !msg.author.bot => {
            handle_message_create(ctx, *msg).await;
        }

        // Do nothing if bot
        Event::MessageCreate(_) => {}

        Event::InteractionCreate(i) => {
            let mut interaction = i.0;
            if interaction.kind == InteractionType::ApplicationCommand {
                if let Some(InteractionData::ApplicationCommand(data)) = interaction.data.take() {
                    if data.name == "download" {
                        download(ctx, interaction, *data).await;
                    }
                }
            }
        }

        Event::Ready(_) => {
            tracing::info!("mie is ready and waiting for your stupid links");
        }
        Event::GatewayHello(_) => {
            tracing::info!("discord said hello");
        }
        _ => {
            tracing::debug!("recieved event, but have no handler {:?}", event);
        }
    }

    Ok(())
}

async fn get_application_id(http: &HttpClient) -> anyhow::Result<Id<ApplicationMarker>> {
    let application_data = http.current_user_application().await?;
    Ok(application_data.model().await?.id)
}
