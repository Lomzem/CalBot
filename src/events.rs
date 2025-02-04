use serenity::{
    all::{
        Context, CreateButton, CreateMessage, EventHandler, Guild, Message, MessageBuilder,
        Permissions, Ready,
    },
    async_trait,
};

use crate::{
    parser::{parse_msg, Error},
    utils::{calendar_message, upload_calendar},
};

pub struct Handler;

#[async_trait]
impl EventHandler for Handler {
    // Set a handler for the `message` event. This is called whenever a new message is received.
    // Event handlers are dispatched through a threadpool, and so multiple events can be
    // dispatched simultaneously.
    async fn message(&self, ctx: Context, msg: Message) {
        let bot_id = ctx.cache.current_user().id;

        if msg.author.id == bot_id {
            // don't care about bot messages
            return;
        }

        if !msg.mentions_user_id(&bot_id) {
            // bot only responds to @CalBot mentions
            return;
        }

        // check that user is an admin
        let guild = Guild::get(&ctx, msg.guild_id.unwrap())
            .await
            .expect("msg should have guild");
        let member = guild
            .member(&ctx, msg.author.id)
            .await
            .expect("msg should have came from member in guild");
        let perms = guild.member_permissions(&member);

        if !perms.administrator() {
            if let Err(why) = msg
                .channel_id
                .say(&ctx, "Sorry! Only admins can use this bot.")
                .await
            {
                println!("Error sending message: {why}");
            }
            return;
        }

        // The bot accepts two inputs
        // 1. A message with information with mentions it with an @CalBot
        // 2. Replying to a message with information and mentioning @CalBot in the reply
        let res = match msg.referenced_message {
            Some(ref ref_msg) => {
                if let Some(edited) = ref_msg.edited_timestamp {
                    parse_msg(&ref_msg.content, &edited.date_naive()).await
                } else {
                    parse_msg(&ref_msg.content, &ref_msg.timestamp.date_naive()).await
                }
            }
            None => parse_msg(&msg.content, &msg.timestamp.date_naive()).await,
        };

        match res {
            Ok(calendar) => {
                let cal_url = upload_calendar(&ctx, &calendar).await;
                let btn = CreateButton::new_link(cal_url).label("Add to iCal");

                let mut cal_msg = MessageBuilder::new();
                calendar_message(&calendar, &mut cal_msg);

                let message = CreateMessage::new()
                    .content(cal_msg.build())
                    .button(btn)
                    .reference_message(&msg);
                if let Err(why) = msg.channel_id.send_message(&ctx, message).await {
                    println!("Error sending message: {why}");
                }
                return;
            }
            Err(Error::ParseFailure) => {
                if let Err(why) = msg
                    .reply(&ctx, "Sorry! I couldn't parse that message.")
                    .await
                {
                    println!("Error sending message: {why}");
                }
            }
            Err(Error::NoResponse) => {
                if let Err(why) = msg
                    .reply(&ctx.http, "Sorry! The LLM didn't respond. Try again later.")
                    .await
                {
                    println!("Error sending message: {why}");
                }
            }
            Err(Error::Reqwest(e)) => {
                println!("Error: {e}");
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
