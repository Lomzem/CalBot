mod events;
mod parser;
mod utils;
use shuttle_runtime::SecretStore;

use events::Handler;
use serenity::prelude::*;

#[shuttle_runtime::main]
async fn serenity(
    #[shuttle_runtime::Secrets] secrets: SecretStore,
) -> shuttle_serenity::ShuttleSerenity {
    // Get the discord token set in `Secrets.toml`
    let token = secrets
        .get("DISCORD_TOKEN")
        .expect("'DISCORD_TOKEN' was not found");

    std::env::set_var("GROQ_API_KEY", secrets.get("GROQ_API_KEY").expect("'GROQ_API_KEY' was not found"));
    std::env::set_var("CALBOT_CHAN", secrets.get("CALBOT_CHAN").expect("'CALBOT_CHAN' was not found"));

    // Set gateway intents, which decides what events the bot will be notified about
    let intents = GatewayIntents::GUILD_MESSAGES | GatewayIntents::MESSAGE_CONTENT;

    let client = Client::builder(&token, intents)
        .event_handler(Handler)
        .await
        .expect("Err creating client");

    Ok(client.into())
}
