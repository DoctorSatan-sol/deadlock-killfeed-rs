use anyhow::Context;
use dotenvy::dotenv;
use regex::Regex;
use scraper::{Html, Selector};
use std::env;
use std::sync::Arc;
use std::time::Duration;
use teloxide::prelude::*;
use teloxide::adaptors::Throttle;
use teloxide::requests::RequesterExt;
use teloxide::adaptors::throttle::Limits;

const URLS: [&str; 3] = [
    "https://kills.deadlock.com/",
    "https://kills.deadlock.com/trap.html",
    "https://kills.deadlock.com/turret.html",
];

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenv().ok();

    let bot_token = env::var("BOT_TOKEN")
        .context("BOT_TOKEN missing")?;

    let chat_id: i64 = env::var("CHAT_ID")
        .context("CHAT_ID missing")?
        .parse()?;

    let bot = Bot::new(bot_token).throttle(Limits::default());

    let client = reqwest::Client::builder()
        .danger_accept_invalid_certs(true)
        .build()?;

    let regex = Arc::new(
        Regex::new(
            r"(?i)(?:^|[^A-Za-z0-9_])(Doctor Satan|DoctorBBC)(?:$|[^A-Za-z0-9_])",
        )?
    );

    for url in URLS {
        tokio::spawn(watch(
            client.clone(),
            bot.clone(),
            chat_id,
            url,
            regex.clone(),
        ));
    }

    std::future::pending::<()>().await;

    Ok(())
}

async fn get_kills(
    client: &reqwest::Client,
    url: &str,
    regex: &Regex,
) -> Result<Vec<String>, anyhow::Error> {
    let response = client
        .get(url)
        .send()
        .await?
        .text()
        .await?;

    let document = Html::parse_document(&response);

    let selector = Selector::parse("tr.mono td")
        .map_err(|err| anyhow::anyhow!("selector parse error: {:?}", err))?;

    let mut kills = Vec::new();

    for element in document.select(&selector) {
        let text = element
            .text()
            .collect::<Vec<_>>()
            .join("")
            .trim()
            .to_string();

        if !text.is_empty() && regex.is_match(&text) {
            kills.push(text);
        }
    }

    Ok(kills)
}

async fn watch(
    client: reqwest::Client,
    bot: Throttle<Bot>,
    chat_id: i64,
    url: &'static str,
    regex: Arc<Regex>,
) {
    let mut last_seen: Option<String> = None;

    loop {
        match get_kills(&client, url, &regex).await {
            Ok(kills) => {
                for text in kills.iter().rev() {
                    if Some(text) == last_seen.as_ref() {
                        break;
                    }

                    if let Err(err) = bot
                        .send_message(ChatId(chat_id), text.clone())
                        .await
                    {
                        eprintln!("telegram error: {}", err);
                    }
                }

                if let Some(last) = kills.last() {
                    last_seen = Some(last.clone());
                }
            }

            Err(err) => {
                eprintln!("watch error [{}]: {}", url, err);
            }
        }

        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}