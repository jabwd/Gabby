use crate::check_msg;
use crate::VoiceManager;
use crate::ChannelRegistry;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::{
    CommandResult,
    macros::command,
};

#[command]
async fn link(ctx: &Context, msg: &Message) -> CommandResult {
    {
        let data_read = ctx.data.read().await;
        let channel_id_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel ID").clone();
        let mut channel_id = channel_id_lock.write().await;
        *channel_id = msg.channel_id.0
    };
    check_msg(msg.channel_id.say(&ctx.http, "Now using this channel for TTS input :3").await);
    Ok(())
}

#[command]
async fn unlink(ctx: &Context, msg: &Message) -> CommandResult {
    {
        let data_read = ctx.data.read().await;
        let channel_id_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel ID").clone();
        let mut channel_id = channel_id_lock.write().await;
        *channel_id = 0
    };
    check_msg(msg.channel_id.say(&ctx.http, "I see how it is, no one wants me to speak (┛ಠ_ಠ)┛彡┻━┻").await);
    Ok(())
}
