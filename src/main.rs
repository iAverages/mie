mod commands;
mod embed;
mod env;
mod event_handlers;
mod upload;
mod video;

use std::collections::HashMap;
use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use backblaze_b2_client::structs::B2Client;
use serde::Serialize;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;
use twilight_gateway::{ConfigBuilder, Event, EventTypeFlags, Intents, Shard, ShardId};
use twilight_http::request::{Request, TryIntoRequest};
use twilight_http::routing::Route;
use twilight_http::Client as HttpClient;
use twilight_model::application::command::{CommandOption, CommandType};
use twilight_model::guild::Permissions;
use twilight_model::id::marker::ApplicationMarker;
use twilight_model::id::Id;
use vesper::prelude::Framework;

use self::commands::download::download;
use self::env::{create_config, load_env, Config};
use self::event_handlers::messsage_create::handle_message_create;

pub struct AppContext {
    config: Config,
    http: Arc<HttpClient>,
    b2: Arc<Mutex<B2Client>>,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Load .env first as RUST_LOG is pulled from there in development
    load_env()?;

    // Initialize the tracing subscriber.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let config = create_config();

    // Use intents to only receive guild message events.
    // Use Configbuilder to ignore unwanted events that
    // get included from intents (such as MESSAGE_UPDATE)
    let shard_config = ConfigBuilder::new(
        config.discord_token.clone(),
        Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT | Intents::DIRECT_MESSAGES,
    )
    .event_types(
        EventTypeFlags::MESSAGE_CREATE
            | EventTypeFlags::GATEWAY_HELLO
            | EventTypeFlags::READY
            | EventTypeFlags::INTERACTION_CREATE,
    )
    .build();

    let mut shard = Shard::with_config(ShardId::ONE, shard_config);

    let b2 = Arc::new(Mutex::new(
        B2Client::new(
            &config.b2_key_id.clone(),
            &config.b2_application_key.clone(),
        )
        .await
        .unwrap(),
    ));

    // HTTP is separate from the gateway, so create a new client.
    let http = Arc::new(HttpClient::new(config.discord_token.clone()));

    // Get the current bots application id to create slash commands
    let app_id = get_application_id(&http)
        .await
        .expect("Failed to get application id for current bot token");

    let app_context = Arc::new(AppContext {
        config: config.clone(),
        http: http.clone(),
        b2: b2.clone(),
    });

    let framework = Arc::new(
        Framework::builder(http.clone(), app_id, app_context.clone())
            .command(download)
            .build(),
    );

    // Register slash commands on discord
    // framework.register_global_commands().await.unwrap();

    // Manually create commands so I can use contexts as it currently
    // is not supported in the released versions
    for cmd in framework.commands.values() {
        let options = cmd
            .arguments
            .iter()
            .map(|a| a.as_option(&framework, cmd))
            .collect::<Vec<_>>();

        let c = reqwest::Client::new();
        let path = Route::SetGlobalCommands {
            application_id: app_id.into(),
        }
        .to_string();

        tracing::info!("creating {} command", cmd.name);
        let a = c
            .post(format!("https://discord.com/api/v10/{}", path))
            .header("Authorization", format!("Bot {}", config.discord_token))
            .json(&GlobalCommandBody {
                application_id: Some(app_id),
                description: Some(cmd.description),
                kind: CommandType::ChatInput,
                name: cmd.name,
                options: Some(options),
                contexts: vec![0, 1, 2],
                integration_types: vec![0, 1],
            })
            .send()
            .await?;
    }
    // Start B2 Auth task to keep auth token updated
    b2_auth_task(config.clone(), b2);

    // Process each event as they come in.
    loop {
        match shard.next_event().await {
            Ok(item) => {
                tokio::spawn(handle_event(
                    Arc::clone(&app_context),
                    framework.clone(),
                    item,
                ));
            }
            Err(err) => {
                tracing::warn!(source = ?err, "error receiving event");
            }
        }
    }
}

#[derive(Serialize)]
struct GlobalCommandBody<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub application_id: Option<Id<ApplicationMarker>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<&'a str>,
    #[serde(rename = "type")]
    pub kind: CommandType,
    pub name: &'a str,
    #[serde(default)]
    pub options: Option<Vec<CommandOption>>,
    pub contexts: Vec<u32>,
    pub integration_types: Vec<u32>,
}

async fn handle_event(
    ctx: Arc<AppContext>,
    framework: Arc<Framework<Arc<AppContext>>>,
    event: Event,
) -> Result<(), Box<dyn Error + Send + Sync>> {
    match event {
        Event::MessageCreate(msg) if !msg.author.bot => {
            handle_message_create(ctx, *msg).await;
        }

        // Do nothing if bot
        Event::MessageCreate(_) => {}

        Event::InteractionCreate(i) => {
            tracing::info!("hello interation");
            framework.process(i.0).await;
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

const TIME_BETWEEN_B2_AUTH: u64 = 79200; // 22 hours

fn b2_auth_task(config: Config, b2_client: Arc<Mutex<B2Client>>) {
    tokio::spawn(async move {
        tracing::info!("starting reauth thread");
        let key_id = &config.b2_key_id.clone();
        let application_key = &config.b2_application_key.clone();
        let b2_client = &b2_client.clone();
        loop {
            tokio::time::sleep(Duration::from_secs(TIME_BETWEEN_B2_AUTH)).await;
            tracing::info!("reauthorizing B2 account");
            b2_client
                .lock()
                .await
                .authorize_account(key_id, application_key)
                .await
                .expect("Could not authorize B2 account");

            tracing::info!("Done! Waiting {} seconds", TIME_BETWEEN_B2_AUTH);
        }
    });
}

async fn get_application_id(http: &HttpClient) -> anyhow::Result<Id<ApplicationMarker>> {
    let application_data = http.current_user_application().await?;
    Ok(application_data.model().await?.id)
}
