use serenity::{builder::CreateEmbed, utils::Colour};

pub struct Embed {
    serenity_embed: serenity::model::channel::Embed,
}

// impl Default for Embed {
//     fn default() -> Self {
//         let mut embed = CreateEmbed::default();
//         embed.color(EMBED_COLOR);

//         Self {
//             serenity_embed: embed.,
//         }
//     }
// }

impl Embed {
    pub fn new(serenity_embed: serenity::model::channel::Embed) -> Self {
        Self { serenity_embed }
    }
}

const EMBED_COLOR: Colour = Colour::new(11762810);
