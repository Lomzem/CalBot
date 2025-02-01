use std::env;

use icalendar::Calendar;
use serenity::all::{ChannelId, Context, CreateAttachment, CreateMessage};

pub async fn upload_calendar(ctx: &Context, calendar: &Calendar) -> String {
    // returns a url to the uploaded .ics file
    let priv_chan = ChannelId::new(
        env::var("CALBOT_CHAN")
            .expect("CALBOT_CHAN missing")
            .parse()
            .expect("Invalid CALBOT_CHAN"),
    );

    let attachment = CreateAttachment::bytes(calendar.to_string().as_bytes(), "CalBot.ics");
    let message = CreateMessage::new().add_file(attachment);

    let sent = priv_chan
        .send_message(ctx, message)
        .await
        .expect("Failed to send message");

    sent.attachments
        .first()
        .expect("Bot should have added an attachment")
        .url
        .to_owned()
}
