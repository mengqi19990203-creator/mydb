use crate::keychain;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AiChatMessage {
    pub role: String,
    pub content: String,
}
use std::time::Duration;

const KEYCHAIN_ID: &str = "mydb-ai";
const KEYCHAIN_KIND: &str = "api-key";

fn endpoint(provider: &str, base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if provider == "anthropic" {
        if base.ends_with("/v1") {
            format!("{base}/messages")
        } else {
            format!("{base}/v1/messages")
        }
    } else {
        format!("{base}/chat/completions")
    }
}

fn provider_name(provider: &str) -> &str {
    match provider {
        "anthropic" => "Claude",
        "openai" => "OpenAI",
        "deepseek" => "DeepSeek",
        "ollama" => "Ollama",
        _ => "自定义 OpenAI 兼容服务",
    }
}

fn sanitize_error(message: &str, api_key: &str) -> String {
    if api_key.is_empty() {
        message.to_string()
    } else {
        message.replace(api_key, "***")
    }
}

fn normalized_history(history: Option<Vec<AiChatMessage>>) -> Vec<AiChatMessage> {
    history
        .unwrap_or_default()
        .into_iter()
        .filter(|message| {
            matches!(message.role.as_str(), "user" | "assistant")
                && !message.content.trim().is_empty()
                && message.content.len() <= 20_000
        })
        .rev()
        .take(20)
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .collect()
}

fn request_messages(
    history: &[AiChatMessage],
    user_prompt: &str,
) -> Vec<Value> {
    let mut messages = history
        .iter()
        .map(|message| {
            json!({
                "role": message.role,
                "content": message.content,
            })
        })
        .collect::<Vec<_>>();
    messages.push(json!({ "role": "user", "content": user_prompt }));
    messages
}

fn text_from_response(provider: &str, body: &Value) -> Result<String, String> {
    if provider == "anthropic" {
        let content = body
            .get("content")
            .and_then(Value::as_array)
            .ok_or_else(|| "Claude 返回格式缺少 content".to_string())?;
        let text = content
            .iter()
            .filter_map(|item| item.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n");
        if text.trim().is_empty() {
            return Err("Claude 返回了空内容".to_string());
        }
        return Ok(text);
    }
    let content = body
        .get("choices")
        .and_then(Value::as_array)
        .and_then(|choices| choices.first())
        .and_then(|choice| choice.get("message"))
        .and_then(|message| message.get("content"))
        .ok_or_else(|| "模型返回格式缺少 choices[0].message.content".to_string())?;
    let text = if let Some(text) = content.as_str() {
        text.to_string()
    } else if let Some(parts) = content.as_array() {
        parts
            .iter()
            .filter_map(|part| part.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n")
    } else {
        return Err("模型返回了无法识别的 content 格式".to_string());
    };
    let text = text.as_str();
    if text.trim().is_empty() {
        return Err("模型返回了空内容".to_string());
    }
    Ok(text.to_string())
}

#[tauri::command]
pub async fn ai_save_api_key(key: String) -> Result<(), String> {
    let key = key.trim();
    if key.is_empty() {
        keychain::delete(KEYCHAIN_ID, KEYCHAIN_KIND);
        Ok(())
    } else {
        keychain::store(KEYCHAIN_ID, KEYCHAIN_KIND, key)
    }
}

#[tauri::command]
pub async fn ai_has_api_key() -> Result<bool, String> {
    Ok(keychain::load(KEYCHAIN_ID, KEYCHAIN_KIND)?.is_some())
}

#[tauri::command]
pub async fn ai_chat(
    provider: String,
    base_url: String,
    model: String,
    system_prompt: String,
    user_prompt: String,
    max_tokens: Option<u32>,
    history: Option<Vec<AiChatMessage>>,
) -> Result<String, String> {
    let provider = provider.trim().to_ascii_lowercase();
    let base_url = base_url.trim();
    let model = model.trim();
    if base_url.is_empty() {
        return Err(format!("请先配置 {} 的 Base URL", provider_name(&provider)));
    }
    if model.is_empty() {
        return Err(format!("请先配置 {} 的模型名称", provider_name(&provider)));
    }
    if user_prompt.trim().is_empty() {
        return Err("请输入需求或 SQL".to_string());
    }
    let api_key = keychain::load(KEYCHAIN_ID, KEYCHAIN_KIND)?.unwrap_or_default();
    if provider != "ollama" && api_key.is_empty() {
        return Err(format!(
            "请先在设置中填写 {} 的 API Key",
            provider_name(&provider)
        ));
    }
    let max_tokens = max_tokens.unwrap_or(2048).clamp(64, 8192);
    chat_with_key(
        &provider,
        base_url,
        model,
        &system_prompt,
        &user_prompt,
        max_tokens,
        history,
        &api_key,
    )
    .await
}

async fn chat_with_key(
    provider: &str,
    base_url: &str,
    model: &str,
    system_prompt: &str,
    user_prompt: &str,
    max_tokens: u32,
    history: Option<Vec<AiChatMessage>>,
    api_key: &str,
) -> Result<String, String> {
    let provider = provider.to_string();
    let history = normalized_history(history);
    let messages = request_messages(&history, user_prompt);
    let openai_messages = {
        let mut all = vec![json!({ "role": "system", "content": system_prompt })];
        all.extend(messages.iter().cloned());
        all
    };
    let url = endpoint(&provider, base_url);
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(90))
        .build()
        .map_err(|error| format!("创建 AI 请求失败：{error}"))?;

    let request = if provider == "anthropic" {
        client
            .post(&url)
            .header("x-api-key", api_key)
            .header("anthropic-version", "2023-06-01")
            .header("content-type", "application/json")
            .json(&json!({
                "model": model,
                "max_tokens": max_tokens,
                "system": system_prompt,
                "messages": messages,
            }))
    } else {
        let mut builder = client
            .post(&url)
            .header("content-type", "application/json")
            .json(&json!({
                "model": model,
                "temperature": 0.2,
                "max_tokens": max_tokens,
                "messages": openai_messages,
            }));
        if provider != "ollama" && !api_key.is_empty() {
            builder = builder.bearer_auth(api_key);
        }
        builder
    };

    let response = request
        .send()
        .await
        .map_err(|error| format!("{} 请求失败：{error}", provider_name(&provider)))?;
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|error| format!("读取 AI 响应失败：{error}"))?;
    let text = sanitize_error(&text, api_key);
    if !status.is_success() {
        return Err(format!(
            "{} 返回 {}：{}",
            provider_name(&provider),
            status.as_u16(),
            text.chars().take(1200).collect::<String>()
        ));
    }
    let body: Value = serde_json::from_str(&text)
        .map_err(|error| format!("解析 AI 响应失败：{error}"))?;
    text_from_response(&provider, &body)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_provider_endpoints() {
        assert_eq!(
            endpoint("anthropic", "https://api.anthropic.com"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint("anthropic", "https://api.anthropic.com/v1/"),
            "https://api.anthropic.com/v1/messages"
        );
        assert_eq!(
            endpoint("openai", "https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            endpoint("ollama", "http://127.0.0.1:11434/v1"),
            "http://127.0.0.1:11434/v1/chat/completions"
        );
    }

    #[test]
    fn reads_openai_and_anthropic_text() {
        let openai = serde_json::json!({
            "choices": [{ "message": { "content": "SELECT 1" } }]
        });
        assert_eq!(text_from_response("openai", &openai).unwrap(), "SELECT 1");
        let anthropic = serde_json::json!({
            "content": [{ "type": "text", "text": "SELECT 2" }]
        });
        assert_eq!(text_from_response("anthropic", &anthropic).unwrap(), "SELECT 2");
    }

    #[tokio::test]
    #[ignore = "requires MYDB_TEST_AI_KEY / MYDB_TEST_AI_BASE / MYDB_TEST_AI_MODEL"]
    async fn real_ai_chat_regression() {
        let key = std::env::var("MYDB_TEST_AI_KEY").expect("AI key env");
        let base = std::env::var("MYDB_TEST_AI_BASE").expect("AI base env");
        let model = std::env::var("MYDB_TEST_AI_MODEL").expect("AI model env");
        let reply = chat_with_key(
            "deepseek",
            &base,
            &model,
            "You are a connectivity test. Reply with exactly OK.",
            "Reply with exactly OK.",
            16,
            None,
            &key,
        )
        .await
        .expect("AI chat should succeed");
        assert!(!reply.trim().is_empty());
    }

    #[test]
    fn redacts_api_key_from_errors() {
        assert_eq!(
            sanitize_error("bad key sk-secret", "sk-secret"),
            "bad key ***"
        );
    }
}
