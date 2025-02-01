use serenity::{
    all::{Context, CreateButton, CreateMessage, EventHandler, Message, Ready},
    async_trait,
};

use crate::{
    parser::{parse_msg, Error},
    utils::upload_calendar,
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        let bot_id = ctx.cache.current_user().id;

        if !msg.mentions_user_id(&bot_id) {
            // bot only responds to @CalBot mentions
            return;
        }

        // The bot accepts two inputs
        // 1. A message with information with mentions it with an @CalBot
        // 2. Replying to a message with information and mentioning @CalBot in the reply
        let res = match msg.referenced_message {
            Some(ref ref_msg) => parse_msg(&ref_msg.content).await,
            None => parse_msg(&msg.content).await,
        };
        match res {
            Ok(calendar) => {
                let cal_url = upload_calendar(&ctx, &calendar).await;
                let btn = CreateButton::new_link(cal_url).label("Add to iCal");
                let message = CreateMessage::new().button(btn).reference_message(&msg);
                if let Err(why) = msg.channel_id.send_message(&ctx, message).await {
                    println!("Error sending message: {why}");
                }
                return;
            }
            Err(e) => {
                if let Some(err) = e.downcast_ref::<Error>() {
                    match err {
                        Error::ParseFailure => {
                            if let Err(why) = msg
                                .reply(&ctx, "Sorry! I couldn't parse that message.")
                                .await
                            {
                                println!("Error sending message: {why}");
                            }
                        }
                        Error::NoResponse => {
                            if let Err(why) = msg
                                .reply(&ctx.http, "Sorry! The LLM didn't respond. Try again later.")
                                .await
                            {
                                println!("Error sending message: {why}");
                            }
                        }
                    }
                } else {
                    println!("Error: {e}");
                }
            }
        }
    }

    // Set a handler to be called on the `ready` event. This is called when a shard is booted, and
    // a READY payload is sent by Discord. This payload contains data like the current user's guild
    // Ids, current user data, private channels, and more.
    //
    // In this case, just print what the current user's username is.
    async fn ready(&self, _: Context, ready: Ready) {
        println!("{} is connected!", ready.user.name);
    }
}
