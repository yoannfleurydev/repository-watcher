mod types;
extern crate dotenv;

use dotenv::dotenv;
use reqwest::Client;
use std::collections::HashMap;
use std::env;

use chrono::DateTime;
use chrono::Duration;
use chrono::Utc;
use serde_json::Value;
use types::Repository;

async fn get_stargazers_count(
    client: &Client,
    repository: String,
) -> Result<String, Box<dyn std::error::Error>> {
    let distant_repository = client
        .get(format!("https://api.github.com/repos/{}", repository))
        .send()
        .await?
        .json::<Repository>()
        .await?;

    Ok(distant_repository.stargazers_count.to_string())
}

fn get_this_week_pull_requests(pulls: &Vec<Value>) -> Vec<Value> {
    let week_ago = Utc::now().checked_sub_signed(Duration::weeks(1)).unwrap();

    pulls
        .clone()
        .into_iter()
        .filter(|pull| {
            let merged_at = pull["merged_at"].as_str();

            match merged_at {
                Some(merged_at) => {
                    let merged_at_date = DateTime::parse_from_rfc3339(merged_at).unwrap();

                    merged_at_date > week_ago
                }
                None => false,
            }
        })
        .filter(|pull| {
            let labels = pull["labels"].as_array().unwrap();
            labels
                .iter()
                .any(|label| label["name"].as_str() == Some("changelog"))
        })
        .collect()
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let slack_hook = env::var("SLACK_HOOK").expect("SLACK_HOOK environment variable not set");
    let repository = env::var("REPOSITORY").expect("REPOSITORY environment variable not set");

    let client = reqwest::Client::builder()
        .user_agent("yoannfleurydev/start-ui-watcher")
        .build()?;

    let pulls = client
        .get(format!(
            "https://api.github.com/repos/{repository}/pulls?state=closed&sort=updated&direction=desc",
        ))
        .send()
        .await?
        .text()
        .await?;

    let root: Value = serde_json::from_str(pulls.as_str())?;

    let pulls = root.as_array().unwrap();
    let this_week_pulls: Vec<Value> = get_this_week_pull_requests(pulls);

    // Map pull requests to print the title and the link
    let important = this_week_pulls
        .iter()
        .map(|pull| {
            let title = pull["title"].as_str().unwrap();
            let url = pull["html_url"].as_str().unwrap();

            format!("<{}|{}>", url, title)
        })
        .collect::<Vec<String>>()
        .join("\n");

    let intro = match important.is_empty() {
        true => "No important Pull Requests this week",
        false => "The important Pull Requests of the week are:",
    };

    let stargazers_count = get_stargazers_count(&client, repository).await?;
    let text = format!(
        r#"
🚀 Start UI [web] :start-ui-web: has *{}* stars.

{}

{}
"#,
        stargazers_count, intro, important
    );
    let mut map = HashMap::new();
    map.insert("text", text);

    client.post(slack_hook).json(&map).send().await?;

    Ok(())
}
