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
    model::{channel::Message, gateway::Ready},
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

use commands::{
    join::*,
    leave::*,
    link::*,
    sound::*,
};

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
        println!("Msg: {:?}", msg);
    }
}

#[group]
#[commands(deafen, join, leave, mute, play, undeafen, unmute, say)]
struct General;

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
    }

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
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

/// Checks that a message successfully sent; if not, then logs why to stdout.
fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
