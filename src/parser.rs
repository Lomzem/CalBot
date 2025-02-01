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
    Reqwest(reqwest::Error),
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::ParseFailure => write!(f, "Failed to parse response from Groq API"),
            Error::NoResponse => write!(f, "No response from Groq API"),
            Error::Reqwest(e) => write!(f, "Reqwest error: {}", e),
        }
    }
}

impl std::error::Error for Error {}

pub async fn parse_msg(msg: &str) -> Result<Calendar, Error> {
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

    let groq_resp: GroqResponse = request
        .send()
        .await
        .map_err(Error::Reqwest)?
        .json()
        .await
        .map_err(Error::Reqwest)?;

    parse_groq_response(groq_resp)
}

fn parse_groq_response(groq_resp: GroqResponse) -> Result<Calendar, Error> {
    if let Some(groq_choice) = groq_resp.choices.first() {
        let output = &groq_choice.message.content;
        if output == "" || output.to_lowercase().contains("failed") {
            return Err(Error::ParseFailure.into());
        }

        Ok(output
            .parse::<Calendar>()
            .map_err(|_| Error::ParseFailure)?)
    } else {
        return Err(Error::NoResponse.into());
    }
}

#[cfg(test)]
mod tests {
    use chrono::Timelike;
    use icalendar::{Component, EventLike};

    use super::*;

    // by default, ignore tests that require a POST request to the Groq API

    #[tokio::test]
    #[ignore]
    async fn test_irrelevant_input() {
        let msg = "69420";
        let res = parse_msg(&msg).await;
        assert!(matches!(res, Err(Error::ParseFailure)));
    }

    #[tokio::test]
    #[ignore]
    async fn test_today_date() {
        let msg = "ACM Club is meeting today from 4-6pm in OCNL 241!";
        let res = parse_msg(&msg).await;

        assert!(matches!(res, Ok(_)));
        let calendar = res.unwrap();
        assert_eq!(calendar.components.len(), 1);
        let event = calendar.components.first().unwrap().as_event().unwrap();

        let location = event.get_location();
        assert!(location.is_some(), "Expected location to be present");
        assert_eq!(location.unwrap(), "OCNL 241");

        let start_dt = event.get_start();
        assert!(start_dt.is_some(), "Expected start date to be present");
        let start_dt = start_dt.unwrap();

        assert!(matches!(start_dt, icalendar::DatePerhapsTime::DateTime(_)));

        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = start_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));

            if let icalendar::CalendarDateTime::Floating(naive_date_time) = calendar_date_time {
                assert_eq!(naive_date_time.date(), chrono::Local::now().date_naive());
                assert_eq!(naive_date_time.time().hour(), 16);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }
    }
}
