use serde::{Deserialize, Serialize};
use anyhow::Result;
use reqwest::Client;
use serde_json::Value;
use log::{debug, info};

use crate::tools::{Tool, ToolCall};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: Vec<ContentBlock>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    #[serde(rename = "type")]
    pub block_type: String,
    pub text: Option<String>,
    pub id: Option<String>,
    pub name: Option<String>,
    pub input: Option<Value>,
    pub tool_use_id: Option<String>,
    pub content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ContentBlock {
    pub fn text(text: String) -> Self {
        Self {
            block_type: "text".to_string(),
            text: Some(text),
            id: None,
            name: None,
            input: None,
            tool_use_id: None,
            content: None,
            is_error: None,
        }
    }

    pub fn tool_use(id: String, name: String, input: Value) -> Self {
        Self {
            block_type: "tool_use".to_string(),
            text: None,
            id: Some(id),
            name: Some(name),
            input: Some(input),
            tool_use_id: None,
            content: None,
            is_error: None,
        }
    }

    pub fn tool_result(tool_use_id: String, content: String, is_error: Option<bool>) -> Self {
        Self {
            block_type: "tool_result".to_string(),
            text: None,
            id: None,
            name: None,
            input: None,
            tool_use_id: Some(tool_use_id),
            content: Some(content),
            is_error,
        }
    }
}

#[derive(Debug, Serialize)]
struct AnthropicRequest {
    model: String,
    max_tokens: u32,
    temperature: f32,
    messages: Vec<Message>,
    tools: Option<Vec<ToolDefinition>>,
    stream: Option<bool>,
}

#[derive(Debug, Serialize)]
struct ToolDefinition {
    name: String,
    description: String,
    input_schema: serde_json::Value,
}

#[derive(Debug, Deserialize)]
pub struct AnthropicResponse {
    pub content: Vec<ContentBlock>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub input_tokens: u32,
    pub output_tokens: u32,
}

impl Usage {
    pub fn total(&self) -> u32 {
        self.input_tokens + self.output_tokens
    }
}

pub struct AnthropicClient {
    client: Client,
    api_key: String,
    base_url: String,
}

impl AnthropicClient {
    pub fn new(api_key: String, base_url: String) -> Self {
        Self {
            client: Client::new(),
            api_key,
            base_url,
        }
    }

    pub async fn create_message(
        &self,
        model: &str,
        messages: Vec<Message>,
        tools: &[Tool],
        max_tokens: u32,
        temperature: f32,
    ) -> Result<AnthropicResponse> {
        // Try the standard endpoint first, then fall back to alternatives if needed
        let endpoints = vec![
            format!("{}/v1/messages", self.base_url),
            format!("{}/messages", self.base_url),
            format!("{}/anthropic/v1/messages", self.base_url),
        ];

        for endpoint in endpoints.iter() {
            match self.try_endpoint(endpoint, model, &messages, tools, max_tokens, temperature).await {
                Ok(response) => {
                    return Ok(response);
                }
                Err(_) => {
                    // Continue to the next endpoint
                    continue;
                }
            }
        }

        // If all endpoints failed, return the error from the last attempt
        let last_endpoint = &endpoints[endpoints.len() - 1];
        return self.try_endpoint(last_endpoint, model, &messages, tools, max_tokens, temperature).await;
    }

    async fn try_endpoint(
        &self,
        endpoint: &str,
        model: &str,
        messages: &[Message],
        tools: &[Tool],
        max_tokens: u32,
        temperature: f32,
    ) -> Result<AnthropicResponse> {
        let tool_definitions = if tools.is_empty() {
            None
        } else {
            Some(tools.iter().map(|t| ToolDefinition {
                name: t.name.clone(),
                description: t.description.clone(),
                input_schema: t.input_schema.clone(),
            }).collect())
        };

        let request = AnthropicRequest {
            model: model.to_string(),
            max_tokens,
            temperature,
            messages: messages.to_vec(),
            tools: tool_definitions,
            stream: Some(false),
        };

        // Log outgoing request
        info!("Sending API request to endpoint: {}", endpoint);
        debug!("Request body: {}", serde_json::to_string_pretty(&request)?);
        info!("Sending message to model: {}", model);

        let response = self.client
            .post(endpoint)
            .header("x-api-key", &self.api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&request)
            .send()
            .await?;

        let status = response.status();
        if !status.is_success() {
            let error_text = response.text().await?;
            return Err(anyhow::anyhow!("API error: {} - {}", status, error_text));
        }

        // Get the response text
        let response_text = response.text().await?;

        // Log incoming response
        info!("Received API response with status: {}", status);
        info!("Response body: {}", response_text);

        // Try to parse the response
        match serde_json::from_str::<AnthropicResponse>(&response_text) {
            Ok(anthropic_response) => {
                info!("Successfully received response from API: {}", response_text);
                if let Some(usage) = &anthropic_response.usage {
                    info!("Token usage - Input: {}, Output: {}", usage.input_tokens, usage.output_tokens);
                }
                Ok(anthropic_response)
            },
            Err(e) => {
                // Try to parse as a generic JSON to handle error responses
                match serde_json::from_str::<serde_json::Value>(&response_text) {
                    Ok(value) => {
                        // Check if this is an error response with specific fields
                        if let (Some(code), Some(msg), Some(success)) = (
                            value.get("code").and_then(|v| v.as_u64()),
                            value.get("msg").and_then(|v| v.as_str()),
                            value.get("success").and_then(|v| v.as_bool())
                        ) {
                            if !success {
                                return Err(anyhow::anyhow!("API Error (HTTP {}): {} - This suggests the endpoint or authentication is incorrect", code, msg));
                            }
                        }

                        Err(anyhow::anyhow!("Failed to parse API response: {} - Invalid response format", e))
                    }
                    Err(_) => {
                        Err(anyhow::anyhow!("Invalid JSON response from API: {}", e))
                    }
                }
            }
        }
    }

    pub fn convert_tool_calls(&self, content_blocks: &[ContentBlock]) -> Vec<ToolCall> {
        content_blocks
            .iter()
            .filter_map(|block| {
                if block.block_type == "tool_use" {
                    Some(ToolCall {
                        id: block.id.as_ref().unwrap_or(&String::new()).clone(),
                        name: block.name.as_ref().unwrap_or(&String::new()).clone(),
                        arguments: block.input.as_ref().unwrap_or(&Value::Null).clone(),
                    })
                } else {
                    None
                }
            })
            .collect()
    }

    pub fn create_response_content(&self, content_blocks: &[ContentBlock]) -> String {
        content_blocks
            .iter()
            .filter_map(|block| {
                if block.block_type == "text" {
                    block.text.clone()
                } else {
                    None
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }
}