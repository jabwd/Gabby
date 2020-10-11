extern crate dotenv;
extern crate base64_stream;

mod commands;
mod tts;

use tts::{
    google_tts::*,
};

use dotenv::dotenv;
use std::{env, sync::Arc};
use serenity::client::bridge::voice::ClientVoiceManager;
use serenity::{client::Context, prelude::Mutex};
use serenity::{
    async_trait,
    client::{Client, EventHandler},
    framework::{
        StandardFramework,
        standard::{
            Args, CommandResult,
            macros::{command, group},
        },
    },
    model::{channel::Message, gateway::Ready},
    Result as SerenityResult,
    voice,
    prelude::*,
};
use std::fs::File;
use std::io::prelude::*;
use std::{collections::HashMap};
use tokio::sync::RwLock;

use commands::{
    join::*,
    leave::*,
    link::*,
    sound::*,
};

#[group]
#[commands(
    deafen,
    undeafen,
    mute,
    unmute,
    join,
    leave,
    say,
    link,
    unlink
)]
struct General;
struct VoiceManager;
struct ChannelRegistry;
struct UserPreferences;
struct Handler;

impl TypeMapKey for ChannelRegistry {
    type Value = Arc<RwLock<u64>>;
}

impl TypeMapKey for UserPreferences {
    type Value = Arc<RwLock<HashMap<u64, String>>>;
}

impl TypeMapKey for VoiceManager {
    type Value = Arc<Mutex<ClientVoiceManager>>;
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name)
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if (msg.author.name == "Gabby") {
            return
        }
        if (msg.content.starts_with("!")) {
            return
        }
        let channel_id = {
            let data_read = ctx.data.read().await;
            let channel_id_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel ID").clone();
            let channel_id = channel_id_lock.read().await;
            *channel_id
        };

        // In case we don't have a channel_id linked we simply can't play any tts
        // so we just ignore the message outright
        if channel_id == 0 {
            return;
        } else if channel_id == msg.channel_id.0 {
            let _ = handle_tts_message(&ctx, &msg).await;
        }
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c
                   .prefix("!"))
        .group(&GENERAL_GROUP);

    let mut client = Client::new(&token)
        .event_handler(Handler)
        .framework(framework)
        .await
        .expect("Err creating client");

    // Obtain a lock to the data owned by the client, and insert the client's
    // voice manager into it. This allows the voice manager to be accessible by
    // event handlers and framework commands.
    {
        let mut data = client.data.write().await;
        data.insert::<VoiceManager>(Arc::clone(&client.voice_manager));
        data.insert::<ChannelRegistry>(Arc::new(RwLock::new(0)));
        data.insert::<UserPreferences>(Arc::new(RwLock::new(HashMap::default())));
    }

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
}

async fn handle_tts_message(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel(msg.channel_id).await {
        Some(channel) => channel.guild_id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "Error finding channel info").await);

            return Ok(());
        },
    };
    let manager_lock = ctx.data.read().await
        .get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    if let Some(handler) = manager.get_mut(guild_id) {
        let res = message_to_speech(&msg.content).await?;

        let mut file = File::create("voice.ogg")?;
        file.write_all(&res)?;

        let source = match voice::ffmpeg("./voice.ogg").await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        };
        handler.play(source);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to speak in").await);
    }

    Ok(())
}

#[command]
async fn say(ctx: &Context, msg: &Message, mut _args: Args) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel(msg.channel_id).await {
        Some(channel) => channel.guild_id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "Error finding channel info").await);

            return Ok(());
        },
    };

    let manager_lock = ctx.data.read().await
        .get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    if let Some(handler) = manager.get_mut(guild_id) {

        let res = message_to_speech(&msg.content).await?;

        let mut file = File::create("voice.ogg")?;
        file.write_all(&res)?;

        let source = match voice::ffmpeg("./voice.ogg").await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        };
        handler.play(source);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to speak in").await);
    }

    Ok(())
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
