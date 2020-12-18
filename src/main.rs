extern crate dotenv;
extern crate base64_stream;

mod commands;
mod tts;

use tts::{
    google_tts::*,
    models::Voice,
};

use regex::Regex;
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
            CommandResult,
            macros::{group},
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
    user::*,
};

#[group]
#[commands(
    deafen,
    undeafen,
    mute,
    unmute,
    join,
    leave,
    link,
    unlink,
    register,
    unregister,
    jump_scare,
)]
struct General;
struct VoiceManager;
struct ChannelRegistry;
struct UserPreferences;
struct Handler;

impl TypeMapKey for ChannelRegistry {
    type Value = Arc<RwLock<HashMap<u64, u64>>>;
}

struct UserPref {
    voice: Voice,
}

impl TypeMapKey for UserPreferences {
    type Value = Arc<RwLock<HashMap<u64, UserPref>>>;
}

impl TypeMapKey for VoiceManager {
    type Value = Arc<Mutex<ClientVoiceManager>>;
}

fn clean_message(msg: &Message) -> String {
    let mut final_str = String::from(&msg.content);
    // Filter any URLs
    let re = Regex::new(r"((\w+://)[-a-zA-Z0-9:@;?&=/%\+\.\*!'\(\),\$_\{\}\^~\[\]`#|]+)").unwrap();
    final_str = re.replace_all(&final_str, "").to_string();

    // Filter channels
    let re = Regex::new(r"(<#[0-9]*>)").unwrap();
    final_str = re.replace_all(&final_str, "").to_string();

    // Filter mentions
    for mention in msg.mentions.iter() {
        let id = mention.id;
        let mention_str = format!("<@!{}>", id);
        final_str = final_str.replace(&mention_str, &mention.name);
    }

    final_str
}

#[async_trait]
impl EventHandler for Handler {
    async fn ready(&self, _ctx: Context, _ready: Ready) {
        println!("=> Startup complete");
    }

    async fn message(&self, ctx: Context, msg: Message) {
        if (msg.author.name == "Gabby") {
            return
        }
        if (msg.content == "bitch") {
            check_msg(msg.channel_id.say(&ctx.http, "Excuse me?! Go fuck yourself").await);
            return
        }
        if (msg.content.starts_with("g/")) {
            return
        }
        if (msg.content.contains("crumpets")) {
            check_msg(msg.channel_id.say(&ctx.http, "Crumpets were buttered").await);
            return
        }

        let guild_id;
        match msg.guild_id {
            Some(v) => guild_id = v,
            None => return
        };
        let data_read = ctx.data.read().await;
        let channel_map_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel mappings").clone();
        let channel_map = channel_map_lock.read().await;
        let channel_id: u64;
        match channel_map.get(&guild_id.0) {
            Some(v) => channel_id = *v,
            None => return
        };

        // We return here early so we get rid of the lock sooner
        if channel_id != msg.channel_id.0 {
            return
        }
        let _ = handle_tts_message(&ctx, &msg).await;
    }
}

#[tokio::main]
async fn main() {
    dotenv().ok();
    println!("Starting Gabby…");
    let token = env::var("DISCORD_TOKEN")
        .expect("Expected a token in the environment");

    let framework = StandardFramework::new()
        .configure(|c| c
                   .prefix("g/"))
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
        data.insert::<ChannelRegistry>(Arc::new(RwLock::new(HashMap::default())));
        data.insert::<UserPreferences>(Arc::new(RwLock::new(HashMap::default())));
    }

    let _ = client.start().await.map_err(|why| println!("Client ended: {:?}", why));
}

async fn handle_tts_message(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id = match ctx.cache.guild_channel(msg.channel_id).await {
        Some(channel) => {
            channel.guild_id
        },
        None => {
            check_msg(msg.channel_id.say(&ctx.http, "Error finding channel info").await);

            return Ok(());
        },
    };
    let data_read = ctx.data.read().await;
    let user_preferences_lock = data_read.get::<UserPreferences>().expect("Unable to read channel ID").clone();
    let user_preferences = user_preferences_lock.read().await;
    if let Some(prefs) = user_preferences.get(&msg.author.id.0) {
        println!("Language code: {:?}", prefs.voice.language_code.to_string());
        println!("Name:          {:?}", prefs.voice.name.to_string());
        println!("SSML Gender:   {:?}", prefs.voice.ssml_gender.to_string());
        println!("Final voice: {:?}", prefs.voice);
        let final_voice = Voice {
            language_code: prefs.voice.language_code.to_string(),
            name: prefs.voice.name.to_string(),
            ssml_gender: prefs.voice.ssml_gender.to_string(),
        };
        
        let manager_lock = data_read.get::<VoiceManager>().cloned().expect("Expected VoiceManager in TypeMap.");
        let mut manager = manager_lock.lock().await;
        if let Some(handler) = manager.get_mut(guild_id) {
            let cleaned_msg = clean_message(&msg);
            let res = message_to_speech(&cleaned_msg, final_voice).await?;
    
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
        }
    } else {
        return Ok(());
    }
    Ok(())
}

fn check_msg(result: SerenityResult<Message>) {
    if let Err(why) = result {
        println!("Error sending message: {:?}", why);
    }
}
