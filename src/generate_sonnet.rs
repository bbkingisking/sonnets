use std::time::Duration;

use crate::config::Config;
use anyhow::{Result, anyhow};
use chrono::{Local, NaiveDateTime};
use log::info;
use reqwest::{Client, header::{self, CONTENT_TYPE, HeaderValue}};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{timeout, sleep};

// Entry point
pub async fn generate_sonnet(conf: &Config, noun: Option<String>) -> Result<Sonnet> {
    // Generate the body for the request
    let body = match generate_body(conf, &noun).await {
        Ok(b) => b,
        Err(e) => return Err(anyhow!("There was an error while generating the body for the Anthropic request: {}", e))
    };

    // Construct headers
    let mut headers = header::HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let mut auth_value = match header::HeaderValue::from_str(&conf.api_key) {
        Ok(v) => v,
        Err(e) => return Err(anyhow!("Could not use the Anthropic API key as a header for the request: {}", e))
    };

    auth_value.set_sensitive(true);
    headers.insert("X-Api-Key", auth_value);

    // Build a client with the headers
    let client = match Client::builder().default_headers(headers).build() {
        Ok(c) => c,
        Err(e) => return Err(anyhow!("Could not build a reqwest client for the Anthropic API request: {}", e))
    };

    // Post a request to the Batches API
    let res = client
        .post("https://api.anthropic.com/v1/messages/batches")
        .json(&body)
        .send()
        .await?
        .text()
        .await?;

    // Parse the response from Batches API
    let batch_response: BatchResponse = match serde_json::from_str(&res) {
        Ok(b) => b,
        Err(e) => return Err(anyhow!("Could not deserialize the response from Anthropic's Batches API: {}. Here is a dump: {}", e, res))
    };

    info!("Batch initialized succesfully, monitoring every 5 minutes for response now…");
    // Poll the Batches API until it is finished
    let sonnet = match poll_batch(&batch_response, conf, noun).await {
        Ok(s) => s,
        Err(e) => return Err(anyhow!("{} while polling the batch for the sonnet.", e))
    };

    Ok(sonnet)
}

// Helper to generate the JSON body for the request
async fn generate_body(conf: &Config, nouns: &Option<String>) -> Result<AnthropicBatch> {
    let generic_user_prompt = AnthropicRequestParamsMessage {
        role: "user".to_string(),
        content: "Compose a sonnet.".to_string(),
    };

    // If there is a noun, create a user prompt with it
    let user_prompt: Option<AnthropicRequestParamsMessage> = match nouns {
        Some(n) => Some(AnthropicRequestParamsMessage {
                    role: "user".to_string(),
                    content: format!("Thematic Anchor:\nThe subject of the sonnet is: {}\nUse this not only as image, but as metaphor, tension, or philosophical springboard.", n),
        }),
        None => None,
    };

    // Initialize the messages
    let mut messages: Vec<AnthropicRequestParamsMessage> = Vec::new();

    // Push the system prompt
    messages.push(generic_user_prompt);

    // If there is a user prompt, push it
    if let Some(up) = user_prompt {
        messages.push(up)
    }

    // Put everything together into a higher struct
    let params = AnthropicRequestParams {
        model: conf.model.clone(),
        max_tokens: 1000u32,
        system: conf.system_prompt.clone(),
        messages,
    };

    // Put everything together into a higher struct
    let requests = vec![AnthropicRequest {
        custom_id: "sonnet".to_string(),
        params,
    }];

    let batch = AnthropicBatch { requests };

    Ok(batch)
}

// After a batch is sent, poll until we get the result and convert it into a Sonnet
async fn poll_batch(batch: &BatchResponse, conf: &Config, noun: Option<String>) -> Result<Sonnet> {
    // Construct headers
    let mut headers = header::HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    let mut auth_value = header::HeaderValue::from_str(&conf.api_key)?;
    auth_value.set_sensitive(true);
    headers.insert("X-Api-Key", auth_value);

    // get a client builder
    let client = reqwest::Client::builder()
        .default_headers(headers)
        .build()?;

    let url = format!(
        "https://api.anthropic.com/v1/messages/batches/{}",
        &batch.id
    );

    let timeout_result = timeout(Duration::from_hours(25), poll_until_complete(&client, &url)).await;

    let batch_response = match timeout_result {
        Ok(poll_result) => {
            // Timeout didn't fire, but polling might have failed
            match poll_result {
                Ok(response) => response,  // Success! We got the batch response
                Err(e) => return Err(anyhow!("Error while polling batch: {}", e)),
            }
        },
        Err(_elapsed) => {
            // Timeout fired - took longer than 25 hours
            return Err(anyhow!("Timeout: batch processing took longer than 25 hours"));
        }
    };

    // If we have exited the loop, it means the generation has ended. We can get the result now
    let Some(results_url) = &batch_response.results_url else {
        return Err(anyhow!("Batch ended but can't find results_url field"))
    };

    // Get the result as a generic JSON Value
    let res: Value = client
        .get(results_url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    // Need to check if it has ended with a success or not
    let Some(r) = res.get("result") else {
        return Err(anyhow!(
            "No 'result' field in the batch response, here is a dump of the result: {}",
            serde_json::to_string_pretty(&res)?
        ));
    };

    let Some(s) = r.get("type") else {
        return Err(anyhow!("Could not get the type of the batch response, here is a dump of the result: {}", serde_json::to_string_pretty(&res)?))
    };

    match s.as_str() {
        Some("succeeded") => {},
        _ => return Err(anyhow!(
            "The batch exited with a non-successful code, here is a dump of the result: {}",
            serde_json::to_string_pretty(&res)?
        ))
    };

    // Batch has succeeded, we can start constructing the sonnet
    // Parse the deserialized generic Value into a BatchResults struct
    let batch_results = serde_json::from_value::<BatchResults>(res).map_err(|e| anyhow!("Could not deserialize the Anthropic response into a BatchResults struct: {}", e))?;

    // Get the actual sonnet from the BatchResults struct
    // Check if the messages vec is not empty
    let Some(message_content) = batch_results.result.message.content.get(0) else {
        return Err(anyhow!("Could not get the actual sonnet from the BatchResults struct."))
    };

    // Get the actual content
    let content = message_content.text.to_owned();

    // Get the author
    let author = batch_results.result.message.model.to_owned();

    // Set created_at to the current time
    let created_at: NaiveDateTime = Local::now().naive_local();

    info!("Batch finished. Sonnet deserialized.");

    Ok(Sonnet {
        author,
        prompt: conf.system_prompt.to_owned(),
        created_at,
        content,
        noun,
    })

}

async fn poll_until_complete(client: &reqwest::Client, url: &str) -> Result<BatchResponse> {
    loop {
        let res: BatchResponse = client
            .get(url)
            .send()
            .await?
            .json()
            .await?;

        if res.processing_status == "ended" {
            return Ok(res);
        }

        sleep(Duration::from_mins(5)).await;
    }
}

// Structs for this module
#[derive(Debug, Serialize, Deserialize)]
struct AnthropicBatch {
    requests: Vec<AnthropicRequest>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequest {
    custom_id: String,
    params: AnthropicRequestParams,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequestParams {
    max_tokens: u32,
    model: String,
    system: String,
    messages: Vec<AnthropicRequestParamsMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct AnthropicRequestParamsMessage {
    role: String,
    content: String,
}

#[derive(Debug, Deserialize, Serialize)]
struct BatchResponse {
    id: String,
    processing_status: String,
    results_url: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct BatchResults {
    custom_id: String,
    result: MessageBatchResult,
}

// This will only match a successful result
#[derive(Debug, Serialize, Deserialize)]
struct MessageBatchResult {
    #[serde(rename = "type")]
    _type: String,
    message: Message,
}

#[derive(Debug, Serialize, Deserialize)]
struct Message {
    model: String,
    usage: MessageUsage,
    content: Vec<MessageContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageContent {
    text: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageUsage {
    input_tokens: u32,
    output_tokens: u32,
}

pub struct Sonnet {
    pub author: String,
    pub prompt: String,
    pub created_at: NaiveDateTime,
    pub content: String,
    pub noun: Option<String>,
}
