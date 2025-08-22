use std::sync::Arc;

use anyhow::Result;
use twilight_model::channel::message::embed::EmbedField;
use twilight_model::channel::message::Embed;
use twilight_model::channel::Message;
use twilight_model::id::marker::{ChannelMarker, MessageMarker};
use twilight_model::id::Id;

use crate::AppContext;

pub struct MieEmbed {
    embed: Embed,
    ctx: Arc<AppContext>,
    message_id: Option<Id<MessageMarker>>,
    channel_id: Id<ChannelMarker>,
}

impl MieEmbed {
    pub fn new(ctx: Arc<AppContext>, channel_id: Id<ChannelMarker>) -> Self {
        MieEmbed {
            embed: Self::default_embed(),
            ctx,
            message_id: None,
            channel_id,
        }
    }

    pub fn title(&mut self, title: String) -> &mut Self {
        self.embed.title = Some(title);
        self
    }

    pub fn add_field(&mut self, field: EmbedField) -> &mut Self {
        self.embed.fields.push(field);
        self
    }

    pub fn update_field(&mut self, index: usize, field: EmbedField) -> &mut Self {
        self.embed.fields[index] = field;
        self
    }

    #[allow(dead_code)]
    pub fn remove_field(&mut self, index: usize) -> &mut Self {
        self.embed.fields.remove(index);
        self
    }

    #[allow(dead_code)]
    pub fn reset_fields(&mut self) -> &mut Self {
        self.embed.fields = vec![];
        self
    }

    pub async fn send_or_update(&mut self) -> Result<Message> {
        if self.message_id.is_some() {
            tracing::debug!(
                message_id = self.message_id.unwrap().to_string(),
                "have message_id, updating existing embed"
            );
            let result = self
                .ctx
                .http
                .update_message(self.channel_id, self.message_id.unwrap())
                .embeds(Some(&[self.embed.clone()]))?
                .await?
                .model()
                .await?;

            return Ok(result);
        }

        tracing::debug!("sending embed for first time");
        let message = self
            .ctx
            .http
            .create_message(self.channel_id)
            .embeds(&[self.embed.clone()])?
            .await?
            .model()
            .await?;

        self.message_id = Some(message.id);
        Ok(message)
    }

    pub fn build(&mut self) -> Embed {
        self.embed.clone()
    }

    fn default_embed() -> Embed {
        Embed {
            author: None,
            color: Some(11762810),
            description: None,
            fields: vec![],
            footer: None,
            image: None,
            kind: String::from("rich"),
            provider: None,
            thumbnail: None,
            timestamp: None,
            title: None,
            url: None,
            video: None,
        }
    }
}
