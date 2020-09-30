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
        let test = "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";

        let mut reader = FromBase64Reader::new(Cursor::new(test));
        let mut buff = Vec::new();
        reader.read_to_end(&mut buff);
        /* let source = match voice::opus(true, reader).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            }
        }; */
        let source = voice::opus(true, buff);

        /* let source = match voice::ytdl(&url).await {
            Ok(source) => source,
            Err(why) => {
                println!("Err starting source: {:?}", why);

                check_msg(msg.channel_id.say(&ctx.http, "Error sourcing ffmpeg").await);

                return Ok(());
            },
        }; */

        // handler.play(source);

        // check_msg(msg.channel_id.say(&ctx.http, "Playing song").await);
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