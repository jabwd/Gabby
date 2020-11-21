use crate::check_msg;
use crate::ChannelRegistry;
use serenity::prelude::*;
use serenity::model::prelude::*;
use serenity::framework::standard::{
    CommandResult,
    macros::command,
};

#[command]
async fn link(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id;
    match msg.guild_id {
        Some(v) => guild_id = v,
        None => return Ok(())
    };

    {
        let data_read = ctx.data.read().await;
        let channel_map_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel map").clone();
        let mut channel_map = channel_map_lock.write().await;
        channel_map.insert(guild_id.0, msg.channel_id.0);
    }
    check_msg(msg.channel_id.say(&ctx.http, "Now using this channel for TTS input :3").await);
    Ok(())
}

#[command]
async fn unlink(ctx: &Context, msg: &Message) -> CommandResult {
    let guild_id;
    match msg.guild_id {
        Some(v) => guild_id = v,
        None => return Ok(())
    };

    {
        let data_read = ctx.data.read().await;
        let channel_map_lock = data_read.get::<ChannelRegistry>().expect("Unable to read channel map").clone();
        let mut channel_map = channel_map_lock.write().await;
        channel_map.remove(&guild_id.0);
    }
    check_msg(msg.channel_id.say(&ctx.http, "I see how it is, no one wants me to speak (┛ಠ_ಠ)┛彡┻━┻").await);
    Ok(())
}
