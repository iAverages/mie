mod embed;
mod env;
mod event_handlers;
mod upload;
mod video;

use std::error::Error;
use std::sync::Arc;
use std::time::Duration;

use backblaze_b2_client::structs::B2Client;
use tokio::sync::Mutex;
use tracing_subscriber::EnvFilter;
use twilight_gateway::{ConfigBuilder, Event, EventTypeFlags, Intents, Shard, ShardId};
use twilight_http::Client as HttpClient;

use self::env::{create_config, load_env, Config};
use self::event_handlers::messsage_create::handle_message_create;

pub struct AppContext {
    config: Config,
    http: HttpClient,
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
        Intents::GUILD_MESSAGES | Intents::MESSAGE_CONTENT,
    )
    .event_types(
        EventTypeFlags::MESSAGE_CREATE | EventTypeFlags::GATEWAY_HELLO | EventTypeFlags::READY,
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
    let app_context = Arc::new(AppContext {
        config: config.clone(),
        http: HttpClient::new(config.discord_token.clone()),
        b2: b2.clone(),
    });

    // Start B2 Auth task to keep auth token updated
    b2_auth_task(config.clone(), b2);

    // Process each event as they come in.
    loop {
        match shard.next_event().await {
            Ok(item) => {
                tokio::spawn(handle_event(Arc::clone(&app_context), item));
            }
            Err(err) => {
                tracing::warn!(source = ?err, "error receiving event");
            }
        }
    }
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
