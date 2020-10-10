extern crate dotenv;
extern crate base64_stream;

mod commands;

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
    model::{channel::Message, gateway::Ready, misc::Mentionable},
    Result as SerenityResult,
    voice,
    prelude::*,
};
use base64_stream::FromBase64Reader;
use std::io::Read;
use std::io::Cursor;
use std::fs::File;
use std::io::prelude::*;
use serde::{Deserialize, Serialize};

struct VoiceManager;

impl TypeMapKey for VoiceManager {
    type Value = Arc<Mutex<ClientVoiceManager>>;
}

struct Handler;

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name)
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if msg.content == "!ping" {
            if let Err(why) = msg.channel_id.say(&ctx.http, "Pong!").await {
                println!("Error sending message: {:?}", why);
            }
        }
    }
}

#[group]
#[commands(deafen, join, leave, mute, play, ping, undeafen, unmute, say)]
struct General;

#[tokio::main]
async fn main() {
    dotenv().ok();

    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c
                   .prefix("~"))
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
    }

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
}

#[command]
async fn deafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel(msg.channel_id).await {
        Some(channel) => channel.guild_id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "DMs not supported").await);

            return Ok(());
        },
    };

    let manager_lock = ctx.data.read().await.get::<VoiceManager>().cloned().unwrap();
    let mut manager = manager_lock.lock().await;

    let handler = match manager.get_mut(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    if handler.self_deaf {
        check_msg(msg.channel_id.say(&ctx.http, "Already deafened").await);
    } else {
        handler.deafen(true);

        check_msg(msg.channel_id.say(&ctx.http, "Deafened").await);
    }

    Ok(())
}

#[command]
async fn join(ctx: &Context, msg: &Message) -> CommandResult {
    let guild = match msg.guild(&ctx.cache).await {
        Some(guild) => guild,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "DMs not supported").await);

            return Ok(());
        }
    };

    let guild_id = guild.id;

    let channel_id = guild
        .voice_states.get(&msg.author.id)
        .and_then(|voice_state| voice_state.channel_id);

    let connect_to = match channel_id {
        Some(channel) => channel,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        }
    };

    let manager_lock = ctx.data.read().await.get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    if manager.join(guild_id, connect_to).is_some() {
        check_msg(msg.channel_id.say(&ctx.http, &format!("Joined {}", connect_to.mention())).await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Error joining the channel").await);
    }

    Ok(())
}

#[command]
async fn leave(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel_field(msg.channel_id, |channel| channel.guild_id).await {
        Some(id) => id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "DMs not supported").await);

            return Ok(());
        },
    };

    let manager_lock = ctx.data.read().await.get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;
    let has_handler = manager.get(guild_id).is_some();

    if has_handler {
        manager.remove(guild_id);

        check_msg(msg.channel_id.say(&ctx.http, "Left voice channel").await);
    } else {
        check_msg(msg.reply(ctx, "Not in a voice channel").await);
    }

    Ok(())
}

#[command]
async fn mute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel_field(msg.channel_id, |channel| channel.guild_id).await {
        Some(id) => id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "DMs not supported").await);

            return Ok(());
        },
    };

    let manager_lock = ctx.data.read().await
        .get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    let handler = match manager.get_mut(guild_id) {
        Some(handler) => handler,
        None => {
            check_msg(msg.reply(ctx, "Not in a voice channel").await);

            return Ok(());
        },
    };

    if handler.self_mute {
        check_msg(msg.channel_id.say(&ctx.http, "Already muted").await);
    } else {
        handler.mute(true);

        check_msg(msg.channel_id.say(&ctx.http, "Now muted").await);
    }

    Ok(())
}

#[command]
async fn ping(context: &Context, msg: &Message) -> CommandResult {
    check_msg(msg.channel_id.say(&context.http, "Pong!").await);

    Ok(())
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VoiceResponse {
    audio_content: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VoiceInput {
    text: String,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct Voice {
    language_code: String,
    name: String,
    ssml_gender: String,
}
#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct AudioConfig {
    audio_encoding: String,
}

#[derive(Serialize, Debug)]
#[serde(rename_all = "camelCase")]
struct VoiceRequest {
    input: VoiceInput,
    voice: Voice,
    audio_config: AudioConfig,
}

#[command]
async fn say(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
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

        let body = VoiceRequest {
            input: VoiceInput {
                text: msg.content.to_string(),
            },
            voice: Voice {
                language_code: "en-US".to_string(),
                name: "en-US-Wavenet-A".to_string(),
                ssml_gender: "FEMALE".to_string(),
            },
            audio_config: AudioConfig {
                audio_encoding: "OGG_OPUS".to_string()
            }
        };
        let client = reqwest::Client::new();
        let key = env::var("GOOGLE_API_KEY").expect("Expected a token in the environment");
        let url = format!("https://texttospeech.googleapis.com/v1/text:synthesize?key={}", key);
        let res = client.post(&url)
            .json(&body)
            .send()
            .await?
            .json::<VoiceResponse>().await?;

        let mut reader = FromBase64Reader::new(Cursor::new(res.audio_content));
        let mut buff = Vec::new();
        reader.read_to_end(&mut buff)?;

        let mut file = File::create("voice.ogg")?;
        file.write_all(&buff)?;

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
async fn play(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let url = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            check_msg(msg.channel_id.say(&ctx.http, "Must provide a URL to a video or audio").await);

            return Ok(());
        },
    };

    if !url.starts_with("http") {
        check_msg(msg.channel_id.say(&ctx.http, "Must provide a valid URL").await);

        return Ok(());
    }

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
        let source = match voice::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        };

        handler.play(source);

        check_msg(msg.channel_id.say(&ctx.http, "Playing song").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to play in").await);
    }

    Ok(())
}

#[command]
async fn undeafen(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel_field(msg.channel_id, |channel| channel.guild_id).await {
        Some(id) => id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "Error finding channel info").await);

            return Ok(());
        },
    };

    let manager_lock = ctx.data.read().await
        .get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    if let Some(handler) = manager.get_mut(guild_id) {
        handler.deafen(false);

        check_msg(msg.channel_id.say(&ctx.http, "Undeafened").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to undeafen in").await);
    }

    Ok(())
}

#[command]
async fn unmute(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel_field(msg.channel_id, |channel| channel.guild_id).await {
        Some(id) => id,
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "Error finding channel info").await);

            return Ok(());
        },
    };
    let manager_lock = ctx.data.read().await
        .get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
    let mut manager = manager_lock.lock().await;

    if let Some(handler) = manager.get_mut(guild_id) {
        handler.mute(false);

        check_msg(msg.channel_id.say(&ctx.http, "Unmuted").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "Not in a voice channel to unmute in").await);
    }

    Ok(())
}

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
