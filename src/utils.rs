use std::env;

use icalendar::{Calendar, CalendarDateTime, Component, DatePerhapsTime, EventLike};
use serenity::all::{ChannelId, Context, CreateAttachment, CreateMessage, MessageBuilder};

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

pub fn calendar_message(calendar: &Calendar, mb: &mut MessageBuilder) {
    let event = calendar
        .components
        .first()
        .expect("Generated Calendar should have an event")
        .as_event()
        .expect("Generated Calendar should have an event");

    let start_dt = if let DatePerhapsTime::DateTime(dt) = event
        .get_start()
        .expect("Parsing should ensure this is Some")
    {
        if let CalendarDateTime::Floating(dt) = dt {
            dt
        } else {
            panic!("Start time should be a floating time");
        }
    } else {
        panic!("Start time should be a DateTime");
    };

    let end_dt = if let DatePerhapsTime::DateTime(dt) =
        event.get_end().expect("Parsing should ensure this is Some")
    {
        if let CalendarDateTime::Floating(dt) = dt {
            dt
        } else {
            panic!("Start time should be a floating time");
        }
    } else {
        panic!("Start time should be a DateTime");
    };

    mb.push_quote_safe("**Event Name**: ")
        .push_line_safe(event.get_summary().expect("Event should have a summary"))
        .push_quote_safe("**Date**: ")
        .push_line_safe(start_dt.date().format("%A, %b %e, %Y").to_string())
        .push_quote_safe("**Start Time**: ")
        .push_line_safe(start_dt.time().format("%l:%M %p").to_string())
        .push_quote_safe("**End Time**: ")
        .push_line_safe(end_dt.time().format("%l:%M %p").to_string())
        .push_quote_safe("**Location**: ")
        .push_line_safe(event.get_location().unwrap_or("None"));
    if let Some(desc) = event.get_description() {
        mb.push_quote_safe("**Description**: ").push_line_safe(desc);
    }
}
