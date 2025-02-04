use chrono::{Datelike, Days, NaiveDate, NaiveTime, Weekday};
use icalendar::{Calendar, Component, Event, EventLike};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;

const GROQ_ENDPOINT: &str = "https://api.groq.com/openai/v1/chat/completions";
const PROMPT_INSTRUCTIONS: &str = include_str!("llm-prompt.txt");
const MAX_COMPLETION_TOKEN: usize = 300;

#[derive(Deserialize, Debug)]
struct GroqResponse {
    choices: Vec<GroqChoice>,
}

#[derive(Deserialize, Debug)]
struct GroqChoice {
    message: GroqMessage,
}

#[derive(Deserialize, Debug)]
struct GroqMessage {
    content: String,
}

#[derive(Deserialize, Debug)]
struct GroqOutput {
    title: String,
    date: String,
    starttime: String,
    endtime: String,
    location: String,
    description: Option<String>,
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

fn parse_date(date_str: &str, msg_date: &NaiveDate) -> Result<NaiveDate, Error> {
    let mut date_iter = date_str.chars();
    match date_iter.next() {
        Some('+') => {
            let days_delta: u64 = date_iter
                .collect::<String>()
                .parse()
                .map_err(|_| Error::ParseFailure)?;
            msg_date
                .checked_add_days(Days::new(days_delta))
                .ok_or(Error::ParseFailure)
        }
        Some('_') => {
            let weekday = match date_iter.collect::<String>().to_lowercase().as_str() {
                "mon" => Weekday::Mon,
                "tue" => Weekday::Tue,
                "wed" => Weekday::Wed,
                "thu" => Weekday::Thu,
                "fri" => Weekday::Fri,
                "sat" => Weekday::Sat,
                "sun" => Weekday::Sun,
                _ => return Err(Error::ParseFailure),
            }
            .num_days_from_monday();
            let orig_weekday = msg_date.weekday().num_days_from_monday();
            dbg!(&orig_weekday);
            dbg!(&weekday);
            let days_delta = if orig_weekday < weekday {
                println!("smaller");
                (weekday - orig_weekday) as u64
            } else {
                println!("bigger");
                (7 - orig_weekday + weekday) as u64
            } as u64;

            msg_date
                .checked_add_days(Days::new(days_delta))
                .ok_or(Error::ParseFailure)
        }
        Some('x') => NaiveDate::parse_from_str(
            &format!("{}{}", msg_date.year(), date_iter.collect::<String>()),
            "%Y%m%d",
        )
        .map_err(|_| Error::ParseFailure),
        _ => return Err(Error::ParseFailure),
    }
}

pub async fn parse_msg(msg: &str, message_date: &NaiveDate) -> Result<Calendar, Error> {
    let groq_key = std::env::var("GROQ_API_KEY").expect("GROQ_API_KEY missing");

    let full_prompt = [PROMPT_INSTRUCTIONS, msg].join("\r\n");

    let req_body = serde_json::json!({
        "model": "llama-3.3-70b-versatile",
        // "model": "llama-3.2-90b-vision-preview",
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

    let output = if let Some(groq_choice) = groq_resp.choices.first() {
        &groq_choice.message.content
    } else {
        return Err(Error::NoResponse.into());
    };

    dbg!(&output);

    if output == "" || output.to_lowercase().contains("failed") {
        return Err(Error::ParseFailure.into());
    }

    let groq_output: GroqOutput = toml::from_str(output).map_err(|_| Error::ParseFailure)?;
    Ok(groq_output
        .to_ical(message_date)
        .map_err(|_| Error::ParseFailure)?)
}

impl GroqOutput {
    fn to_ical(&self, message_date: &NaiveDate) -> Result<Calendar, Error> {
        let date = parse_date(&self.date, &message_date)?;
        let starttime =
            NaiveTime::parse_from_str(&self.starttime, "%H%M").map_err(|_| Error::ParseFailure)?;
        let endtime =
            NaiveTime::parse_from_str(&self.endtime, "%H%M").map_err(|_| Error::ParseFailure)?;
        let description = match &self.description {
            Some(desc) => desc,
            None => "",
        };

        let title = self
            .title
            .split_whitespace()
            .map(|word| {
                let mut titlecase = String::new();
                let mut word_chars = word.chars();
                if let Some(first) = word_chars.next() {
                    titlecase.push(first.to_ascii_uppercase());
                }
                titlecase.push_str(&word_chars.collect::<String>());
                titlecase
            })
            .collect::<Vec<String>>()
            .join(" ");

        let event = Event::new()
            .summary(&title)
            .starts(date.and_time(starttime))
            .ends(date.and_time(endtime))
            .description(description)
            .location(&self.location)
            .done();
        Ok(Calendar::new().push(event).done())
    }
}

#[cfg(test)]
mod tests {
    use chrono::{Days, Local, Timelike};
    use icalendar::{Component, EventLike};

    use super::*;

    #[tokio::test]
    async fn parse_date_relwd_early_late() {
        let date_msg = "_thu";
        let date = NaiveDate::from_ymd_opt(2025, 2, 4).unwrap();
        let res = parse_date(&date_msg, &date).unwrap();
        let intended_date = NaiveDate::from_ymd_opt(2025, 2, 6).unwrap();
        assert_eq!(res, intended_date);
    }

    #[tokio::test]
    async fn parse_date_relwd_late_early() {
        let date_msg = "_tue";
        let date = NaiveDate::from_ymd_opt(2025, 2, 1).unwrap();
        let res = parse_date(&date_msg, &date).unwrap();
        let intended_date = NaiveDate::from_ymd_opt(2025, 2, 4).unwrap();
        assert_eq!(res, intended_date);
    }

    #[tokio::test]
    async fn parse_date_relwd_same_day_mon() {
        let date_msg = "_mon";
        let date = NaiveDate::from_ymd_opt(2025, 2, 3).unwrap();
        let res = parse_date(&date_msg, &date).unwrap();
        let intended_date = NaiveDate::from_ymd_opt(2025, 2, 10).unwrap();
        assert_eq!(res, intended_date);
    }

    #[tokio::test]
    async fn parse_date_relwd_same_day_thu() {
        let date_msg = "_thu";
        let date = NaiveDate::from_ymd_opt(2025, 2, 6).unwrap();
        let res = parse_date(&date_msg, &date).unwrap();
        let intended_date = NaiveDate::from_ymd_opt(2025, 2, 13).unwrap();
        assert_eq!(res, intended_date);
    }

    #[tokio::test]
    async fn parse_date_relwd_same_day_sun() {
        let date_msg = "_sun";
        let date = NaiveDate::from_ymd_opt(2025, 2, 9).unwrap();
        let res = parse_date(&date_msg, &date).unwrap();
        let intended_date = NaiveDate::from_ymd_opt(2025, 2, 16).unwrap();
        assert_eq!(res, intended_date);
    }

    // by default, ignore tests that require a POST request to the Groq API

    #[tokio::test]
    #[ignore]
    async fn mock_irrelevant_input() {
        let msg = "69420";
        let res = parse_msg(&msg, &Local::now().date_naive()).await;
        assert!(matches!(res, Err(Error::ParseFailure)));
    }

    #[tokio::test]
    #[ignore]
    async fn mock_today_date() {
        let msg = "ACM Club is meeting today from 4-6pm in OCNL 241!";
        let date = Local::now().date_naive();
        let res = parse_msg(&msg, &date).await;

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
                assert_eq!(naive_date_time.date(), date);
                assert_eq!(naive_date_time.time().hour(), 16);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));
            if let icalendar::CalendarDateTime::Floating(naive_date_time) = calendar_date_time {
                assert_eq!(naive_date_time.date(), date);
                assert_eq!(naive_date_time.time().hour(), 18);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mock_tmrw_historical_leap_year() {
        let msg = "ACM Club is meeting tomorrow from 4-6pm in OCNL 241!";
        let date = NaiveDate::from_ymd_opt(2024, 2, 28).unwrap();
        let res = parse_msg(&msg, &date).await;

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
                assert_eq!(naive_date_time.date(), date.succ_opt().unwrap());
                assert_eq!(naive_date_time.time().hour(), 16);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));
            if let icalendar::CalendarDateTime::Floating(naive_date_time) = calendar_date_time {
                assert_eq!(naive_date_time.date(), date.succ_opt().unwrap());
                assert_eq!(naive_date_time.time().hour(), 18);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mock_2_days_historical_leap_year() {
        let msg = "ACM Club is meeting in two days from 4-6pm in OCNL 241!";
        let final_date = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
        let res = parse_msg(&msg, &final_date.checked_sub_days(Days::new(2)).unwrap()).await;

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
                assert_eq!(naive_date_time.date(), final_date);
                assert_eq!(naive_date_time.time().hour(), 16);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));
            if let icalendar::CalendarDateTime::Floating(naive_date_time) = calendar_date_time {
                assert_eq!(naive_date_time.date(), final_date);
                assert_eq!(naive_date_time.time().hour(), 18);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mock_5_days_historical_leap_year() {
        let msg = "ACM Club is meeting in five days from 5-7pm in OCNL 241!";
        let final_date = NaiveDate::from_ymd_opt(2020, 2, 29).unwrap();
        let res = parse_msg(&msg, &final_date.checked_sub_days(Days::new(5)).unwrap()).await;

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
                assert_eq!(naive_date_time.date(), final_date);
                assert_eq!(naive_date_time.time().hour(), 17);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));
            if let icalendar::CalendarDateTime::Floating(naive_date_time) = calendar_date_time {
                assert_eq!(naive_date_time.date(), final_date);
                assert_eq!(naive_date_time.time().hour(), 19);
                assert_eq!(naive_date_time.time().minute(), 0);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mock_missing_end_time() {
        let msg = "ACM Club is meeting in tomorrow at 4pm in OCNL 241!";
        let date = NaiveDate::from_ymd_opt(2021, 6, 9).unwrap();
        let res = parse_msg(&msg, &date).await;

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
            assert!(
                matches!(calendar_date_time, icalendar::CalendarDateTime::Floating(_)),
                "{}",
                format!("{:?}", calendar_date_time)
            );

            if let icalendar::CalendarDateTime::Floating(start_datetime) = calendar_date_time {
                assert_eq!(start_datetime.date(), date.succ_opt().unwrap());
                assert_eq!(start_datetime.time().hour(), 16);
                assert_eq!(start_datetime.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));

            if let icalendar::CalendarDateTime::Floating(end_datetime) = calendar_date_time {
                assert_eq!(end_datetime.date(), date.succ_opt().unwrap());
                assert_eq!(end_datetime.time().hour(), 17);
                assert_eq!(end_datetime.time().minute(), 0);
            }
        }
    }

    #[tokio::test]
    #[ignore]
    async fn mock_exact_date() {
        let msg = "ACM Club is meeting on 10/31 from 11:30-2:45pm in the Mechoopda Dorms";
        let date = NaiveDate::from_ymd_opt(2009, 6, 9).unwrap();
        let res = parse_msg(&msg, &date).await;

        assert!(matches!(res, Ok(_)));
        let calendar = res.unwrap();
        assert_eq!(calendar.components.len(), 1);
        let event = calendar.components.first().unwrap().as_event().unwrap();

        let location = event.get_location();
        assert!(location.is_some(), "Expected location to be present");
        assert_eq!(location.unwrap(), "Mechoopda Dorms");

        let start_dt = event.get_start();
        assert!(start_dt.is_some(), "Expected start date to be present");
        let start_dt = start_dt.unwrap();
        assert!(matches!(start_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = start_dt {
            assert!(
                matches!(calendar_date_time, icalendar::CalendarDateTime::Floating(_)),
                "{}",
                format!("{:?}", calendar_date_time)
            );

            if let icalendar::CalendarDateTime::Floating(start_datetime) = calendar_date_time {
                assert_eq!(
                    start_datetime.date(),
                    NaiveDate::from_ymd_opt(2009, 10, 31).unwrap()
                );
                assert_eq!(start_datetime.time().hour(), 11);
                assert_eq!(start_datetime.time().minute(), 30);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));

            if let icalendar::CalendarDateTime::Floating(end_datetime) = calendar_date_time {
                assert_eq!(
                    end_datetime.date(),
                    NaiveDate::from_ymd_opt(2009, 10, 31).unwrap()
                );
                assert_eq!(end_datetime.time().hour(), 14);
                assert_eq!(end_datetime.time().minute(), 45);
            }
        }
    }
    #[tokio::test]
    #[ignore]
    async fn real_usr0_1_28_25() {
        let msg = "Hey @everyone Voting has concluded and it has been decided that our meeting time this semester will be Mondays from 5-6 in OCNL 239.  Our first meeting will be next Monday where we will be discussing the schedule for the upcoming semester, and doing some intro into hacking and cybersecurity.";
        let date = NaiveDate::from_ymd_opt(2025, 1, 28).unwrap();
        let res = parse_msg(&msg, &date).await;

        assert!(matches!(res, Ok(_)));
        let calendar = res.unwrap();
        assert_eq!(calendar.components.len(), 1);
        let event = calendar.components.first().unwrap().as_event().unwrap();

        let location = event.get_location();
        assert!(location.is_some(), "Expected location to be present");
        assert_eq!(location.unwrap(), "OCNL 239");

        let start_dt = event.get_start();
        assert!(start_dt.is_some(), "Expected start date to be present");
        let start_dt = start_dt.unwrap();
        assert!(matches!(start_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = start_dt {
            assert!(
                matches!(calendar_date_time, icalendar::CalendarDateTime::Floating(_)),
                "{}",
                format!("{:?}", calendar_date_time)
            );

            if let icalendar::CalendarDateTime::Floating(start_datetime) = calendar_date_time {
                assert_eq!(
                    start_datetime.date(),
                    NaiveDate::from_ymd_opt(2025, 2, 3).unwrap()
                );
                assert_eq!(start_datetime.time().hour(), 17);
                assert_eq!(start_datetime.time().minute(), 0);
            }
        }

        let end_dt = event.get_end();
        assert!(end_dt.is_some(), "Expected end date to be present");
        let end_dt = end_dt.unwrap();
        assert!(matches!(end_dt, icalendar::DatePerhapsTime::DateTime(_)));
        if let icalendar::DatePerhapsTime::DateTime(calendar_date_time) = end_dt {
            assert!(matches!(
                calendar_date_time,
                icalendar::CalendarDateTime::Floating(_)
            ));

            if let icalendar::CalendarDateTime::Floating(end_datetime) = calendar_date_time {
                assert_eq!(
                    end_datetime.date(),
                    NaiveDate::from_ymd_opt(2025, 2, 3).unwrap()
                );
                assert_eq!(end_datetime.time().hour(), 18);
                assert_eq!(end_datetime.time().minute(), 0);
            }
        }
    }
}
