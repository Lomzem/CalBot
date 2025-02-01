use chrono::Local;
use icalendar::Calendar;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const PROMPT_INSTRUCTIONS: &str = include_str!("llm-prompt.txt");
const FORMAT: &str = include_str!("ics-format.ics");
const MAX_COMPLETION_TOKEN: usize = 300;

#[derive(Deserialize)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Deserialize)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Deserialize)]
struct GroqMessage {
    content: String,
}

#[derive(Debug)]
pub enum Error {
    ParseFailure,
    NoResponse,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseFailure => write!(f, "Failed to parse response from Groq API"),
            Error::NoResponse => write!(f, "No response from Groq API"),
        }
    }
}

impl std::error::Error for Error {}

pub async fn parse_msg(msg: &str) -> anyhow::Result<Calendar> {
    // OK Variant returns a string representing the
    // content of a valid .ics file
    let groq_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY missing");

    let cur_date_str = format!(
        "If dates are relative, assume the current date is {}",
        Local::now().format("%Y-%m-%d").to_string()
    );

    let full_prompt = [PROMPT_INSTRUCTIONS, FORMAT, &cur_date_str, msg].join("\r\n");

    let req_body = serde_json::json!({
        "model": "llama-3.3-70b-versatile",
        "max_completion_tokens": MAX_COMPLETION_TOKEN,
        "messages": [
        {
            "role": "user",
            "content": full_prompt,
        }
    ]});

    let request = reqwest::Client::new()
        .post(GROQ_ENDPOINT)
        .json(&req_body)
        .header(CONTENT_TYPE, "application/json")
        .header(AUTHORIZATION, format!("Bearer {}", groq_key));

    let groq_resp: GroqResponse = request.send().await?.json().await?;

    if let Some(groq_choice) = groq_resp.choices.first() {
        let output = &groq_choice.message.content;
        if output == "" || output.contains("failed") {
            return Err(Error::ParseFailure.into());
        }

        Ok(output
            .parse::<Calendar>()
            .map_err(|_| Error::ParseFailure)?)
    } else {
        return Err(Error::NoResponse.into());
    }
}
