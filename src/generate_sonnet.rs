use std::time::Duration;

use crate::config::Config;
use anyhow::{Result, anyhow};
use chrono::{Local, NaiveDateTime};
use log::{debug, info, warn};
use rand::seq::SliceRandom;
use reqwest::{
    Client,
    header::{self, CONTENT_TYPE, HeaderValue},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::time::{sleep, timeout};

const ALLOWED_FORMS: [&str; 8] = ["ballad", "epigram", "haiku", "sijo", "rubai", "heroic 10-line couplet", "terza rima", "limerick"];

// Entry point
pub async fn generate_sonnet(
    conf: &Config,
    noun: Option<String>,
    inspiration: Option<String>,
) -> Result<Sonnet> {
    debug!(
        "Starting sonnet generation: noun_configured={}, inspiration_characters={:?}, model={:?}.",
        noun.is_some(),
        inspiration.as_ref().map(|text| text.len()),
        conf.model
    );
    // Generate the body for the request
    let body = generate_body(conf, noun.as_deref(), inspiration.as_deref());
    debug!(
        "Generated Anthropic batch request: requests={}, prompt_characters={:?}.",
        body.requests.len(),
        body.requests
            .first()
            .and_then(|request| request.params.messages.first())
            .map(|message| message.content.len())
    );

    // Construct headers
    let mut headers = header::HeaderMap::new();
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));

    let mut auth_value = match header::HeaderValue::from_str(&conf.api_key) {
        Ok(v) => v,
        Err(e) => {
            return Err(anyhow!(
                "Could not use the Anthropic API key as a header for the request: {}",
                e
            ));
        }
    };

    auth_value.set_sensitive(true);
    headers.insert("X-Api-Key", auth_value);

    // Build a client with the headers
    let client = match Client::builder().default_headers(headers).build() {
        Ok(c) => c,
        Err(e) => {
            return Err(anyhow!(
                "Could not build a reqwest client for the Anthropic API request: {}",
                e
            ));
        }
    };
    debug!("Anthropic HTTP client constructed; submitting batch request.");

    // Post a request to the Batches API
    let response = client
        .post("https://api.anthropic.com/v1/messages/batches")
        .json(&body)
        .send()
        .await?;
    debug!(
        "Anthropic batch submission returned HTTP {}.",
        response.status()
    );
    let res = response.text().await?;
    debug!(
        "Anthropic batch submission response contains {} characters.",
        res.len()
    );

    // Parse the response from Batches API
    let batch_response: BatchResponse = match serde_json::from_str(&res) {
        Ok(b) => b,
        Err(e) => {
            return Err(anyhow!(
                "Could not deserialize the response from Anthropic's Batches API: {}. Here is a dump: {}",
                e,
                res
            ));
        }
    };
    debug!(
        "Batch accepted: id={:?}, processing_status={:?}, results_url_present={}",
        batch_response.id,
        batch_response.processing_status,
        batch_response.results_url.is_some()
    );

    info!("Batch initialized succesfully, monitoring every 5 minutes for response now…");
    // Poll the Batches API until it is finished
    let sonnet = match poll_batch(&batch_response, conf, noun).await {
        Ok(s) => s,
        Err(e) => return Err(anyhow!("{} while polling the batch for the sonnet.", e)),
    };

    Ok(sonnet)
}

// Helper to generate the JSON body for the request
fn generate_body(conf: &Config, noun: Option<&str>, inspiration: Option<&str>) -> AnthropicBatch {
    let mut prompt = String::from("Compose a poem.");

    if let Some(noun) = noun {
        prompt.push_str(&format!(
            "\n\nThematic anchor:\nThe subject of the sonnet is: {noun}\nUse it not only as an image, but as a metaphor, tension, or philosophical springboard."
        ));
    }

    if let Some(inspiration) = inspiration {
        prompt.push_str(&format!(
            "\n\nThe following poems are provided solely as stylistic inspiration. Draw from their mood, voice, and techniques, but do not copy their wording, imagery, or structure.\n\n<inspiration_poems>\n{inspiration}\n</inspiration_poems>"
        ));
    }

    let mut allowed_forms = ALLOWED_FORMS;
    let mut rng = rand::rng();
    allowed_forms.shuffle(&mut rng);
    let random_form: &str = allowed_forms[0];
    debug!("Random form was selected as: \"{}\"", random_form);

    prompt.push_str(&format!("\n\nYou are writing in the {} form", random_form));

    debug!(
        "Building prompt: noun_included={}, inspiration_included={}, total_characters={}",
        noun.is_some(),
        inspiration.is_some(),
        prompt.len()
    );

    let messages = vec![AnthropicRequestParamsMessage {
        role: "user".to_string(),
        content: prompt,
    }];

    // Put everything together into a higher struct
    let params = AnthropicRequestParams {
        model: conf.model.clone(),
        // Adaptive thinking tokens count against this total. Leave enough room for
        // both a short reasoning pass and the sonnet itself.
        max_tokens: 2048u32,
        // Claude Fable has adaptive thinking permanently enabled. Its depth is
        // controlled with `output_config.effort`, rather than a thinking budget.
        output_config: OutputConfig {
            effort: "low".to_string(),
        },
        system: conf.system_prompt.clone(),
        messages,
    };

    // Put everything together into a higher struct
    let requests = vec![AnthropicRequest {
        custom_id: "sonnet".to_string(),
        params,
    }];

    AnthropicBatch { requests }
}

// After a batch is sent, poll until we get the result and convert it into a Sonnet
async fn poll_batch(batch: &BatchResponse, conf: &Config, noun: Option<String>) -> Result<Sonnet> {
    debug!("Preparing to poll batch {:?}.", batch.id);
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

    let timeout_result =
        timeout(Duration::from_hours(25), poll_until_complete(&client, &url)).await;

    let batch_response = match timeout_result {
        Ok(poll_result) => {
            // Timeout didn't fire, but polling might have failed
            match poll_result {
                Ok(response) => response, // Success! We got the batch response
                Err(e) => return Err(anyhow!("Error while polling batch: {}", e)),
            }
        }
        Err(_elapsed) => {
            // Timeout fired - took longer than 25 hours
            return Err(anyhow!(
                "Timeout: batch processing took longer than 25 hours"
            ));
        }
    };
    debug!(
        "Batch {:?} ended with status {:?}; results_url_present={}",
        batch_response.id,
        batch_response.processing_status,
        batch_response.results_url.is_some()
    );

    // If we have exited the loop, it means the generation has ended. We can get the result now
    let Some(results_url) = &batch_response.results_url else {
        return Err(anyhow!("Batch ended but can't find results_url field"));
    };

    // Get the result as a generic JSON Value
    debug!("Fetching completed batch results from {:?}.", results_url);
    let response = client.get(results_url).send().await?;
    debug!("Batch results request returned HTTP {}.", response.status());
    let res: Value = response.error_for_status()?.json().await?;

    // Need to check if it has ended with a success or not
    let Some(r) = res.get("result") else {
        return Err(anyhow!(
            "No 'result' field in the batch response, here is a dump of the result: {}",
            serde_json::to_string_pretty(&res)?
        ));
    };

    let Some(s) = r.get("type") else {
        return Err(anyhow!(
            "Could not get the type of the batch response, here is a dump of the result: {}",
            serde_json::to_string_pretty(&res)?
        ));
    };

    match s.as_str() {
        Some("succeeded") => debug!("Batch result reports success."),
        _ => {
            return Err(anyhow!(
                "The batch exited with a non-successful code, here is a dump of the result: {}",
                serde_json::to_string_pretty(&res)?
            ));
        }
    };

    // Batch has succeeded, we can start constructing the sonnet
    // Parse the deserialized generic Value into a BatchResults struct
    let batch_results = serde_json::from_value::<BatchResults>(res).map_err(|e| {
        anyhow!(
            "Could not deserialize the Anthropic response into a BatchResults struct: {}",
            e
        )
    })?;

    // A response can begin with a thinking block. Select the visible text block
    // rather than assuming the first block is text.
    let Some(content) = batch_results
        .result
        .message
        .content
        .iter()
        .find(|block| block.kind == "text")
        .and_then(|block| block.text.as_deref())
    else {
        return Err(anyhow!(
            "Batch succeeded but returned no text block (stop_reason: {:?}, output_tokens: {}).",
            batch_results.result.message.stop_reason,
            batch_results.result.message.usage.output_tokens,
        ));
    };

    // Get the author
    let author = batch_results.result.message.model.to_owned();
    debug!(
        "Extracted sonnet text: author={:?}, content_characters={}, input_tokens={}, output_tokens={}, stop_reason={:?}.",
        author,
        content.len(),
        batch_results.result.message.usage.input_tokens,
        batch_results.result.message.usage.output_tokens,
        batch_results.result.message.stop_reason
    );

    // Set created_at to the current time
    let created_at: NaiveDateTime = Local::now().naive_local();

    info!("Batch finished. Sonnet deserialized.");

    Ok(Sonnet {
        author,
        prompt: conf.system_prompt.to_owned(),
        created_at,
        content: content.to_owned(),
        noun,
    })
}

async fn poll_until_complete(client: &reqwest::Client, url: &str) -> Result<BatchResponse> {
    let mut poll_count = 0u32;
    loop {
        poll_count += 1;
        let res: BatchResponse = client.get(url).send().await?.json().await?;
        debug!(
            "Batch poll #{}: id={:?}, status={:?}, results_url_present={}",
            poll_count,
            res.id,
            res.processing_status,
            res.results_url.is_some()
        );

        if res.processing_status == "ended" {
            return Ok(res);
        }

        warn!("Batch is still processing; next poll in 5 minutes.");
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
    output_config: OutputConfig,
    system: String,
    messages: Vec<AnthropicRequestParamsMessage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct OutputConfig {
    effort: String,
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
    stop_reason: Option<String>,
    content: Vec<MessageContent>,
}

#[derive(Debug, Serialize, Deserialize)]
struct MessageContent {
    #[serde(rename = "type")]
    kind: String,
    text: Option<String>,
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
