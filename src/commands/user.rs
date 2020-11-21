use crate::check_msg;
use crate::UserPreferences;
use crate::UserPref;
use crate::tts::google_tts::{list_voices};
use crate::tts::models::Voice;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::{
    CommandResult,
    Args,
    macros::command,
};

#[command]
pub async fn register(ctx: &Context, msg: &Message, mut args: Args) -> CommandResult {
    let voices = list_voices().await?;
    let voice_name = match args.single::<String>() {
        Ok(url) => url,
        Err(_) => {
            let mut response = String::from("You need to select a voice (use !register {voice}), here is everything I can do:\n");
            for voice in voices.iter() {
                if voice.language_codes.len() > 0 {
                    let language_code = &voice.language_codes[0];
                    if language_code == "en-US" || language_code == "en-GB" {
                        response.push_str(&format!("> {}: {}\n", voice.ssml_gender.to_lowercase(), voice.name));
                    }
                }
            }
            response.push_str("{voice} is something like en-US-Wavenet-I -- you do not need to provide the gender part");
            check_msg(msg.channel_id.say(&ctx.http, response).await);

            return Ok(());
        },
    };
    let mut voice_iter = voices.into_iter();
    if let Some(voice) = voice_iter.find(|x| x.name == voice_name.trim()) {
        let user_preferences_lock = {
            let data_read = ctx.data.read().await;
            data_read.get::<UserPreferences>().expect("Unable to read channel ID").clone()
        };

        {
            let actual_voice = Voice {
                language_code: voice.language_codes[0].to_string(),
                name: voice.name.to_string(),
                ssml_gender: voice.ssml_gender.to_string(),
            };
            let mut user_preferences = user_preferences_lock.write().await;
            user_preferences.entry(msg.author.id.0).or_insert(UserPref {
                voice: actual_voice
            });
        }
        check_msg(msg.channel_id.say(&ctx.http, "Done! I'll can now speak on your behalf. I'll always be honest and never twist your words at all… >:D").await);
    } else {
        check_msg(msg.channel_id.say(&ctx.http, "I don't know that voice :7").await);
    }
    Ok(())
}

#[command]
pub async fn unregister(ctx: &Context, msg: &Message, mut _args: Args) -> CommandResult {
    let user_preferences_lock = {
        let data_read = ctx.data.read().await;
        data_read.get::<UserPreferences>().expect("Unable to read channel ID").clone()
    };
    
    {
        let mut user_preferences = user_preferences_lock.write().await;
        user_preferences.remove(&msg.author.id.0);
    }
    check_msg(msg.channel_id.say(&ctx.http, "Done! I'll leave your messages alone").await);
    Ok(())
}
