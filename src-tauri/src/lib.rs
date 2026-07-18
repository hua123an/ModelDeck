use chrono::Utc;
use reqwest::{Client, Response};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Mutex,
    time::Instant,
};
use tauri::{AppHandle, Manager, State};
use uuid::Uuid;

const KEYRING_SERVICE: &str = "com.huaan.modeldeck";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum ProviderType {
    NewApi,
    Sub2api,
    OpenaiCompatible,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Provider {
    id: String,
    name: String,
    #[serde(rename = "type")]
    provider_type: ProviderType,
    base_url: String,
    enabled: bool,
    created_at: String,
    updated_at: String,
    has_api_key: bool,
    #[serde(default)]
    has_account_token: bool,
    #[serde(default)]
    account_user_id: Option<String>,
    #[serde(default)]
    has_refresh_token: bool,
    #[serde(default)]
    token_expires_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ProviderInput {
    id: Option<String>,
    name: String,
    #[serde(rename = "type")]
    provider_type: ProviderType,
    base_url: String,
    api_key: Option<String>,
    account_token: Option<String>,
    refresh_token: Option<String>,
    token_expires_at: Option<String>,
    account_user_id: Option<String>,
    enabled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ModelStatus {
    provider_id: String,
    provider_name: String,
    model_id: String,
    name: String,
    available: bool,
    api: String,
    latency_ms: Option<u128>,
    last_checked_at: Option<String>,
    error: Option<String>,
    status_code: Option<u16>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolProfile {
    id: String,
    name: String,
    tool: String,
    provider_id: String,
    model: Option<String>,
    executable: Option<String>,
    active: bool,
    created_at: String,
    updated_at: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ToolProfileInput {
    id: Option<String>,
    name: String,
    tool: String,
    provider_id: String,
    model: Option<String>,
    executable: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct LaunchPreview {
    profile_id: String,
    tool: String,
    executable: String,
    isolated_home: Option<String>,
    environment: Vec<String>,
    untouched_paths: Vec<String>,
    command_preview: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BalanceInfo {
    provider_id: String,
    supported: bool,
    balance: Option<f64>,
    quota: Option<f64>,
    used: Option<f64>,
    reset_at: Option<String>,
    multiplier: Option<f64>,
    account_name: Option<String>,
    frozen_balance: Option<f64>,
    group: Option<String>,
    subscription_name: Option<String>,
    subscription_status: Option<String>,
    error: Option<String>,
    checked_at: String,
}

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PersistedData {
    providers: Vec<Provider>,
    models: Vec<ModelStatus>,
    balances: Vec<BalanceInfo>,
    #[serde(default)]
    profiles: Vec<ToolProfile>,
}

struct AppState(Mutex<PersistedData>);

fn normalize_url(url: &str) -> String {
    let trimmed = url.trim().trim_end_matches('/');
    trimmed.strip_suffix("/v1").unwrap_or(trimmed).to_string()
}

fn data_file(app: &AppHandle) -> Result<PathBuf, String> {
    let dir = app.path().app_data_dir().map_err(|e| e.to_string())?;
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    Ok(dir.join("modeldeck.json"))
}

fn load_data(app: &AppHandle) -> PersistedData {
    data_file(app)
        .ok()
        .and_then(|path| fs::read_to_string(path).ok())
        .and_then(|value| serde_json::from_str(&value).ok())
        .unwrap_or_default()
}

fn save_data(app: &AppHandle, data: &PersistedData) -> Result<(), String> {
    let serialized = serde_json::to_string_pretty(data).map_err(|e| e.to_string())?;
    fs::write(data_file(app)?, serialized).map_err(|e| e.to_string())
}

fn legacy_keyring_entry(provider_id: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, provider_id).map_err(|e| e.to_string())
}

fn keyring_entry(provider_id: &str, slot: &str) -> Result<keyring::Entry, String> {
    keyring::Entry::new(KEYRING_SERVICE, &format!("{provider_id}:{slot}"))
        .map_err(|e| e.to_string())
}

fn get_model_key(provider_id: &str) -> Result<String, String> {
    if let Ok(key) = keyring_entry(provider_id, "model-api-key")?.get_password() {
        return Ok(key);
    }
    legacy_keyring_entry(provider_id)?
        .get_password()
        .map_err(|_| "模型 API Key 不存在或无法从系统钥匙串读取".to_string())
}

fn get_account_token(provider_id: &str) -> Result<String, String> {
    keyring_entry(provider_id, "account-token")?
        .get_password()
        .map_err(|_| "账户访问令牌不存在，请编辑服务商后填写".to_string())
}

fn auth(client: &Client, url: &str, key: &str) -> reqwest::RequestBuilder {
    client
        .get(url)
        .bearer_auth(key)
        .header("Accept", "application/json")
}

async fn response_json(response: Response) -> (u16, Value) {
    let status = response.status().as_u16();
    let text = response.text().await.unwrap_or_default();
    let value = serde_json::from_str(&text).unwrap_or_else(|_| json!({ "raw": text }));
    (status, value)
}

fn value_number(value: &Value, paths: &[&[&str]]) -> Option<f64> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_f64().or_else(|| current.as_str()?.parse().ok())
    })
}

fn value_string(value: &Value, paths: &[&[&str]]) -> Option<String> {
    paths.iter().find_map(|path| {
        let mut current = value;
        for key in *path {
            current = current.get(*key)?;
        }
        current.as_str().map(ToString::to_string)
    })
}

#[tauri::command]
fn list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    Ok(state.0.lock().map_err(|e| e.to_string())?.providers.clone())
}

#[tauri::command]
fn get_snapshot(state: State<'_, AppState>) -> Result<PersistedData, String> {
    let data = state.0.lock().map_err(|e| e.to_string())?;
    Ok(PersistedData {
        providers: data.providers.clone(),
        models: data.models.clone(),
        balances: data.balances.clone(),
        profiles: data.profiles.clone(),
    })
}

#[tauri::command]
fn save_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ProviderInput,
) -> Result<Provider, String> {
    if input.name.trim().is_empty() || input.base_url.trim().is_empty() {
        return Err("服务商名称和 Base URL 不能为空".into());
    }
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    let now = Utc::now().to_rfc3339();
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let existing = data.providers.iter().find(|item| item.id == id).cloned();
    if let Some(key) = input.api_key.filter(|key| !key.trim().is_empty()) {
        keyring_entry(&id, "model-api-key")?
            .set_password(key.trim())
            .map_err(|e| e.to_string())?;
    }
    if let Some(token) = input.account_token.filter(|token| !token.trim().is_empty()) {
        keyring_entry(&id, "account-token")?
            .set_password(token.trim())
            .map_err(|e| e.to_string())?;
    }
    if let Some(token) = input.refresh_token.filter(|token| !token.trim().is_empty()) {
        keyring_entry(&id, "refresh-token")?
            .set_password(token.trim())
            .map_err(|e| e.to_string())?;
    }
    let provider = Provider {
        id: id.clone(),
        name: input.name.trim().to_string(),
        provider_type: input.provider_type,
        base_url: normalize_url(&input.base_url),
        enabled: input.enabled,
        created_at: existing
            .as_ref()
            .map(|p| p.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
        has_api_key: keyring_entry(&id, "model-api-key")?.get_password().is_ok()
            || legacy_keyring_entry(&id)?.get_password().is_ok(),
        has_account_token: keyring_entry(&id, "account-token")?.get_password().is_ok(),
        account_user_id: input
            .account_user_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty()),
        has_refresh_token: keyring_entry(&id, "refresh-token")?.get_password().is_ok(),
        token_expires_at: input
            .token_expires_at
            .or_else(|| existing.and_then(|p| p.token_expires_at)),
    };
    if let Some(position) = data.providers.iter().position(|item| item.id == id) {
        data.providers[position] = provider.clone();
    } else {
        data.providers.push(provider.clone());
    }
    save_data(&app, &data)?;
    Ok(provider)
}

#[tauri::command]
fn delete_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<(), String> {
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    data.providers.retain(|item| item.id != provider_id);
    data.models.retain(|item| item.provider_id != provider_id);
    data.balances.retain(|item| item.provider_id != provider_id);
    for entry in [
        legacy_keyring_entry(&provider_id),
        keyring_entry(&provider_id, "model-api-key"),
        keyring_entry(&provider_id, "account-token"),
        keyring_entry(&provider_id, "refresh-token"),
    ] {
        let _ = entry.and_then(|value| value.delete_credential().map_err(|e| e.to_string()));
    }
    save_data(&app, &data)
}

#[tauri::command]
fn toggle_provider(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
    enabled: bool,
) -> Result<(), String> {
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    let provider = data
        .providers
        .iter_mut()
        .find(|item| item.id == provider_id)
        .ok_or("服务商不存在")?;
    provider.enabled = enabled;
    provider.updated_at = Utc::now().to_rfc3339();
    save_data(&app, &data)
}

#[tauri::command]
async fn fetch_models(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ModelStatus>, String> {
    let provider = {
        let data = state.0.lock().map_err(|e| e.to_string())?;
        data.providers
            .iter()
            .find(|item| item.id == provider_id)
            .cloned()
            .ok_or("服务商不存在")?
    };
    let key = get_model_key(&provider.id)?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let started = Instant::now();
    let response = auth(&client, &format!("{}/v1/models", provider.base_url), &key)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let latency = started.elapsed().as_millis();
    let (status, body) = response_json(response).await;
    if !(200..300).contains(&status) {
        return Err(format!(
            "HTTP {status}: {}",
            body.get("error")
                .and_then(|e| e.get("message"))
                .and_then(Value::as_str)
                .unwrap_or("模型列表请求失败")
        ));
    }
    let items = body
        .get("data")
        .and_then(Value::as_array)
        .or_else(|| body.as_array())
        .ok_or("响应成功，但没有可识别的模型列表")?;
    let checked_at = Utc::now().to_rfc3339();
    let models: Vec<ModelStatus> = items
        .iter()
        .filter_map(|item| {
            let model_id = item
                .get("id")
                .or_else(|| item.get("model"))
                .and_then(Value::as_str)?
                .to_string();
            let name = item
                .get("name")
                .or_else(|| item.get("display_name"))
                .and_then(Value::as_str)
                .unwrap_or(&model_id)
                .to_string();
            Some(ModelStatus {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                model_id,
                name,
                available: true,
                api: "unknown".into(),
                latency_ms: Some(latency),
                last_checked_at: Some(checked_at.clone()),
                error: None,
                status_code: Some(status),
            })
        })
        .collect();
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    data.models.retain(|item| item.provider_id != provider.id);
    data.models.extend(models.clone());
    save_data(&app, &data)?;
    Ok(models)
}

async fn try_responses(
    client: &Client,
    provider: &Provider,
    key: &str,
    model_id: &str,
) -> (u16, u128, Value) {
    let started = Instant::now();
    let result = client
        .post(format!("{}/v1/responses", provider.base_url))
        .bearer_auth(key)
        .json(&json!({
            "model": model_id, "input": "Reply with exactly: ok", "max_output_tokens": 16
        }))
        .send()
        .await;
    match result {
        Ok(response) => {
            let latency = started.elapsed().as_millis();
            let (status, body) = response_json(response).await;
            (status, latency, body)
        }
        Err(error) => (
            0,
            started.elapsed().as_millis(),
            json!({"error": {"message": error.to_string()}}),
        ),
    }
}

async fn try_chat(
    client: &Client,
    provider: &Provider,
    key: &str,
    model_id: &str,
) -> (u16, u128, Value) {
    let started = Instant::now();
    let result = client.post(format!("{}/v1/chat/completions", provider.base_url)).bearer_auth(key).json(&json!({
        "model": model_id, "messages": [{"role": "user", "content": "Reply with exactly: ok"}], "max_tokens": 16, "temperature": 0
    })).send().await;
    match result {
        Ok(response) => {
            let latency = started.elapsed().as_millis();
            let (status, body) = response_json(response).await;
            (status, latency, body)
        }
        Err(error) => (
            0,
            started.elapsed().as_millis(),
            json!({"error": {"message": error.to_string()}}),
        ),
    }
}

fn error_message(body: &Value, fallback: &str) -> String {
    body.get("error")
        .and_then(|e| e.get("message"))
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .unwrap_or(fallback)
        .to_string()
}

#[tauri::command]
async fn test_model(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
    model_id: String,
) -> Result<ModelStatus, String> {
    let provider = {
        let data = state.0.lock().map_err(|e| e.to_string())?;
        data.providers
            .iter()
            .find(|item| item.id == provider_id)
            .cloned()
            .ok_or("服务商不存在")?
    };
    let key = get_model_key(&provider.id)?;
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(45))
        .build()
        .map_err(|e| e.to_string())?;
    let (responses_status, responses_latency, responses_body) =
        try_responses(&client, &provider, &key, &model_id).await;
    let (available, api, status_code, latency, error) = if (200..300).contains(&responses_status) {
        (true, "responses", responses_status, responses_latency, None)
    } else {
        let (chat_status, chat_latency, chat_body) =
            try_chat(&client, &provider, &key, &model_id).await;
        if (200..300).contains(&chat_status) {
            (true, "chat-completions", chat_status, chat_latency, None)
        } else {
            let message = format!(
                "responses: {}; chat: {}",
                error_message(&responses_body, "请求失败"),
                error_message(&chat_body, "请求失败")
            );
            (
                false,
                "unknown",
                if chat_status > 0 {
                    chat_status
                } else {
                    responses_status
                },
                chat_latency,
                Some(message),
            )
        }
    };
    let result = ModelStatus {
        provider_id: provider.id.clone(),
        provider_name: provider.name.clone(),
        model_id: model_id.clone(),
        name: model_id.clone(),
        available,
        api: api.into(),
        latency_ms: Some(latency),
        last_checked_at: Some(Utc::now().to_rfc3339()),
        error,
        status_code: Some(status_code),
    };
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    if let Some(item) = data
        .models
        .iter_mut()
        .find(|item| item.provider_id == provider.id && item.model_id == model_id)
    {
        let name = item.name.clone();
        *item = result.clone();
        item.name = name;
    } else {
        data.models.push(result.clone());
    }
    save_data(&app, &data)?;
    Ok(result)
}

async fn account_get(
    client: &Client,
    provider: &Provider,
    token: &str,
    endpoint: &str,
    user_id: Option<&str>,
) -> Result<(u16, Value), String> {
    let mut request = auth(client, &format!("{}{}", provider.base_url, endpoint), token);
    if let Some(value) = user_id {
        request = request.header("New-Api-User", value);
    }
    let response = request.send().await.map_err(|e| e.to_string())?;
    Ok(response_json(response).await)
}

fn empty_balance(provider_id: &str, error: String) -> BalanceInfo {
    BalanceInfo {
        provider_id: provider_id.to_string(),
        supported: false,
        balance: None,
        quota: None,
        used: None,
        reset_at: None,
        multiplier: None,
        account_name: None,
        frozen_balance: None,
        group: None,
        subscription_name: None,
        subscription_status: None,
        error: Some(error),
        checked_at: Utc::now().to_rfc3339(),
    }
}

async fn query_new_api_balance(client: &Client, provider: &Provider, token: &str) -> BalanceInfo {
    let Some(user_id) = provider.account_user_id.as_deref() else {
        return empty_balance(&provider.id, "New API 余额查询还需要用户 ID".into());
    };
    let (status, body) =
        match account_get(client, provider, token, "/api/user/self", Some(user_id)).await {
            Ok(value) => value,
            Err(error) => return empty_balance(&provider.id, error),
        };
    if !(200..300).contains(&status) || body.get("success").and_then(Value::as_bool) == Some(false)
    {
        return empty_balance(
            &provider.id,
            format!(
                "HTTP {status}: {}",
                error_message(&body, "账户信息请求失败")
            ),
        );
    }
    let raw_quota = value_number(&body, &[&["data", "quota"]]);
    if raw_quota.is_none() {
        return empty_balance(
            &provider.id,
            "账户接口请求成功，但响应中没有 quota 字段".into(),
        );
    }
    let quota_per_unit = match account_get(client, provider, token, "/api/status", None).await {
        Ok((status, config)) if (200..300).contains(&status) => {
            value_number(&config, &[&["data", "quota_per_unit"]]).unwrap_or(500_000.0)
        }
        _ => 500_000.0,
    };
    let divisor = if quota_per_unit > 0.0 {
        quota_per_unit
    } else {
        500_000.0
    };
    let quota = raw_quota.map(|value| value / divisor);
    let used = value_number(&body, &[&["data", "used_quota"]]).map(|value| value / divisor);
    let group = value_string(&body, &[&["data", "group"]]);
    let mut multiplier = None;
    if let Ok((group_status, groups)) = account_get(
        client,
        provider,
        token,
        "/api/user/self/groups",
        Some(user_id),
    )
    .await
    {
        if (200..300).contains(&group_status) {
            if let Some(group_name) = group.as_deref() {
                multiplier = value_number(&groups, &[&["data", group_name, "ratio"]]);
            }
        }
    }
    BalanceInfo {
        provider_id: provider.id.clone(),
        supported: true,
        balance: None,
        quota,
        used,
        reset_at: None,
        multiplier,
        account_name: value_string(&body, &[&["data", "display_name"], &["data", "username"]]),
        frozen_balance: None,
        group,
        subscription_name: None,
        subscription_status: None,
        error: None,
        checked_at: Utc::now().to_rfc3339(),
    }
}

async fn query_sub2api_balance(client: &Client, provider: &Provider, token: &str) -> BalanceInfo {
    let (status, body) =
        match account_get(client, provider, token, "/api/v1/user/profile", None).await {
            Ok(value) => value,
            Err(error) => return empty_balance(&provider.id, error),
        };
    if !(200..300).contains(&status)
        || body
            .get("code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0)
    {
        return empty_balance(
            &provider.id,
            format!(
                "HTTP {status}: {}",
                error_message(&body, "账户 JWT 无效或已过期")
            ),
        );
    }
    let balance = value_number(&body, &[&["data", "balance"]]);
    if balance.is_none() {
        return empty_balance(
            &provider.id,
            "账户接口请求成功，但响应中没有 balance 字段".into(),
        );
    }
    let mut subscription_name = None;
    let mut subscription_status = None;
    let mut reset_at = None;
    if let Ok((sub_status, subscriptions)) = account_get(
        client,
        provider,
        token,
        "/api/v1/subscriptions/active",
        None,
    )
    .await
    {
        if (200..300).contains(&sub_status) {
            if let Some(first) = subscriptions
                .get("data")
                .and_then(Value::as_array)
                .and_then(|items| items.first())
            {
                subscription_name =
                    value_string(first, &[&["group", "name"], &["group_name"], &["name"]]);
                subscription_status = value_string(first, &[&["status"]]);
                reset_at = value_string(first, &[&["expires_at"], &["end_at"]]);
            }
        }
    }
    BalanceInfo {
        provider_id: provider.id.clone(),
        supported: true,
        balance,
        quota: None,
        used: None,
        reset_at,
        multiplier: None,
        account_name: value_string(&body, &[&["data", "username"], &["data", "email"]]),
        frozen_balance: value_number(&body, &[&["data", "frozen_balance"]]),
        group: None,
        subscription_name,
        subscription_status,
        error: None,
        checked_at: Utc::now().to_rfc3339(),
    }
}

async fn query_compatible_balance(client: &Client, provider: &Provider, key: &str) -> BalanceInfo {
    let endpoints = [
        "/dashboard/billing/credit_grants",
        "/v1/dashboard/billing/credit_grants",
    ];
    let mut last_error = "暂不支持该兼容站余额查询".to_string();
    for endpoint in endpoints {
        let (status, body) = match account_get(client, provider, key, endpoint, None).await {
            Ok(value) => value,
            Err(error) => {
                last_error = error;
                continue;
            }
        };
        if !(200..300).contains(&status) {
            last_error = format!("{endpoint}: HTTP {status}");
            continue;
        }
        let balance = value_number(&body, &[&["total_available"], &["balance"]]);
        if balance.is_some() {
            return BalanceInfo {
                provider_id: provider.id.clone(),
                supported: true,
                balance,
                quota: value_number(&body, &[&["total_granted"]]),
                used: value_number(&body, &[&["total_used"]]),
                reset_at: value_string(&body, &[&["expires_at"]]),
                multiplier: None,
                account_name: None,
                frozen_balance: None,
                group: None,
                subscription_name: None,
                subscription_status: None,
                error: None,
                checked_at: Utc::now().to_rfc3339(),
            };
        }
        last_error = format!("{endpoint}: 请求成功，但没有可识别的余额字段");
    }
    empty_balance(&provider.id, last_error)
}

#[tauri::command]
async fn query_balance(
    app: AppHandle,
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<BalanceInfo, String> {
    let provider = {
        let data = state.0.lock().map_err(|e| e.to_string())?;
        data.providers
            .iter()
            .find(|item| item.id == provider_id)
            .cloned()
            .ok_or("服务商不存在")?
    };
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;
    let result = match provider.provider_type {
        ProviderType::NewApi => match get_account_token(&provider.id) {
            Ok(token) => query_new_api_balance(&client, &provider, &token).await,
            Err(error) => empty_balance(&provider.id, error),
        },
        ProviderType::Sub2api => match get_account_token(&provider.id) {
            Ok(token) => query_sub2api_balance(&client, &provider, &token).await,
            Err(error) => empty_balance(&provider.id, error),
        },
        ProviderType::OpenaiCompatible => match get_model_key(&provider.id) {
            Ok(key) => query_compatible_balance(&client, &provider, &key).await,
            Err(error) => empty_balance(&provider.id, error),
        },
    };
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    data.balances.retain(|item| item.provider_id != provider.id);
    data.balances.push(result.clone());
    save_data(&app, &data)?;
    Ok(result)
}

#[tauri::command]
fn storage_info(app: AppHandle) -> Result<Value, String> {
    Ok(
        json!({ "dataFile": data_file(&app)?, "keyStorage": "操作系统钥匙串（Keychain / Credential Manager / Secret Service）" }),
    )
}

#[tauri::command]
fn export_pi_models(app: AppHandle, state: State<'_, AppState>) -> Result<String, String> {
    let data = state.0.lock().map_err(|e| e.to_string())?;
    let providers: Vec<Value> = data.providers.iter().filter(|provider| provider.enabled).map(|provider| {
        let models: Vec<Value> = data.models.iter().filter(|model| model.provider_id == provider.id).map(|model| json!({ "id": model.model_id, "name": model.name, "api": model.api })).collect();
        json!({ "id": provider.id, "name": provider.name, "baseUrl": format!("{}/v1", provider.base_url), "models": models, "apiKey": format!("MODELDECK_KEY_{}", provider.id.replace('-', "_").to_uppercase()) })
    }).collect();
    let output = app
        .path()
        .download_dir()
        .map_err(|e| e.to_string())?
        .join("models.json");
    fs::write(
        &output,
        serde_json::to_string_pretty(&json!({ "providers": providers }))
            .map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().to_string())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct AccountSummary {
    provider_id: String,
    provider_name: String,
    provider_type: ProviderType,
    supported: bool,
    account_name: Option<String>,
    balance: Option<f64>,
    used: Option<f64>,
    frozen_balance: Option<f64>,
    request_count: Option<f64>,
    group: Option<String>,
    multiplier: Option<f64>,
    subscription_name: Option<String>,
    subscription_status: Option<String>,
    expires_at: Option<String>,
    token_expires_at: Option<String>,
    error: Option<String>,
    checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedGroup {
    provider_id: String,
    id: String,
    name: String,
    description: Option<String>,
    multiplier: Option<f64>,
    subscription: bool,
    available: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedKey {
    provider_id: String,
    provider_name: String,
    id: String,
    name: String,
    masked_key: String,
    enabled: bool,
    group_id: Option<String>,
    group_name: Option<String>,
    quota: Option<f64>,
    used: Option<f64>,
    unlimited: bool,
    expires_at: Option<String>,
    last_used_at: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ManagedKeyInput {
    provider_id: String,
    id: Option<String>,
    name: String,
    enabled: bool,
    group_id: Option<String>,
    quota: Option<f64>,
    unlimited: bool,
    expires_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct UsageSummary {
    provider_id: String,
    provider_name: String,
    total_requests: f64,
    total_tokens: Option<f64>,
    total_cost: Option<f64>,
    today_requests: Option<f64>,
    today_tokens: Option<f64>,
    today_cost: Option<f64>,
    checked_at: String,
}

async fn refresh_sub2_token(provider: &Provider, client: &Client) -> Result<String, String> {
    let refresh = keyring_entry(&provider.id, "refresh-token")?
        .get_password()
        .map_err(|_| "账户 JWT 已过期，且未配置 Refresh Token".to_string())?;
    let response = client
        .post(format!("{}/api/v1/auth/refresh", provider.base_url))
        .json(&json!({"refresh_token": refresh}))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let (status, body) = response_json(response).await;
    if !(200..300).contains(&status)
        || body
            .get("code")
            .and_then(Value::as_i64)
            .is_some_and(|code| code != 0)
    {
        return Err(error_message(&body, "刷新账户令牌失败"));
    }
    let token =
        value_string(&body, &[&["data", "access_token"]]).ok_or("刷新响应缺少 access_token")?;
    keyring_entry(&provider.id, "account-token")?
        .set_password(&token)
        .map_err(|e| e.to_string())?;
    if let Some(next_refresh) = value_string(&body, &[&["data", "refresh_token"]]) {
        keyring_entry(&provider.id, "refresh-token")?
            .set_password(&next_refresh)
            .map_err(|e| e.to_string())?;
    }
    Ok(token)
}

async fn management_request(
    client: &Client,
    provider: &Provider,
    method: reqwest::Method,
    path: &str,
    payload: Option<Value>,
) -> Result<Value, String> {
    let user_id = provider.account_user_id.as_deref();
    let mut token = get_account_token(&provider.id)?;
    for attempt in 0..2 {
        let url = format!("{}{}", provider.base_url, path);
        let mut request = client
            .request(method.clone(), url)
            .bearer_auth(&token)
            .header("Accept", "application/json");
        if let Some(value) = user_id {
            request = request.header("New-Api-User", value);
        }
        if let Some(ref body) = payload {
            request = request.json(body);
        }
        let response = request.send().await.map_err(|e| e.to_string())?;
        let (status, body) = response_json(response).await;
        if status == 401 && matches!(provider.provider_type, ProviderType::Sub2api) && attempt == 0
        {
            token = refresh_sub2_token(provider, client).await?;
            continue;
        }
        if !(200..300).contains(&status) {
            return Err(format!(
                "HTTP {status}: {}",
                error_message(&body, "管理接口请求失败")
            ));
        }
        if body.get("success").and_then(Value::as_bool) == Some(false)
            || body
                .get("code")
                .and_then(Value::as_i64)
                .is_some_and(|code| code != 0)
        {
            return Err(error_message(&body, "管理接口返回失败"));
        }
        return Ok(body);
    }
    Err("账户令牌刷新后仍无法访问".into())
}

fn get_provider(state: &State<'_, AppState>, provider_id: &str) -> Result<Provider, String> {
    state
        .0
        .lock()
        .map_err(|e| e.to_string())?
        .providers
        .iter()
        .find(|p| p.id == provider_id)
        .cloned()
        .ok_or("服务商不存在".into())
}

fn management_client() -> Result<Client, String> {
    Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn sync_account(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<AccountSummary, String> {
    let provider = get_provider(&state, &provider_id)?;
    if matches!(provider.provider_type, ProviderType::OpenaiCompatible) {
        return Ok(AccountSummary {
            provider_id,
            provider_name: provider.name,
            provider_type: provider.provider_type,
            supported: false,
            account_name: None,
            balance: None,
            used: None,
            frozen_balance: None,
            request_count: None,
            group: None,
            multiplier: None,
            subscription_name: None,
            subscription_status: None,
            expires_at: None,
            token_expires_at: None,
            error: Some("兼容站没有统一账户管理协议".into()),
            checked_at: Utc::now().to_rfc3339(),
        });
    }
    let client = management_client()?;
    let (profile_path, subscription_path) = match provider.provider_type {
        ProviderType::NewApi => ("/api/user/self", Some("/api/subscription/self")),
        ProviderType::Sub2api => ("/api/v1/user/profile", Some("/api/v1/subscriptions/active")),
        _ => unreachable!(),
    };
    let profile =
        management_request(&client, &provider, reqwest::Method::GET, profile_path, None).await?;
    let data = profile.get("data").unwrap_or(&profile);
    let divisor = if matches!(provider.provider_type, ProviderType::NewApi) {
        match management_request(
            &client,
            &provider,
            reqwest::Method::GET,
            "/api/status",
            None,
        )
        .await
        {
            Ok(config) => {
                value_number(&config, &[&["data", "quota_per_unit"]]).unwrap_or(500_000.0)
            }
            Err(_) => 500_000.0,
        }
    } else {
        1.0
    };
    let subscription = if let Some(path) = subscription_path {
        management_request(&client, &provider, reqwest::Method::GET, path, None)
            .await
            .ok()
    } else {
        None
    };
    let first_sub = subscription
        .as_ref()
        .and_then(|v| v.get("data"))
        .and_then(|v| {
            if v.is_array() {
                v.as_array().and_then(|a| a.first())
            } else {
                Some(v)
            }
        });
    let group = value_string(data, &[&["group"]]);
    let multiplier = if matches!(provider.provider_type, ProviderType::NewApi) {
        if let Ok(groups) = management_request(
            &client,
            &provider,
            reqwest::Method::GET,
            "/api/user/self/groups",
            None,
        )
        .await
        {
            group
                .as_deref()
                .and_then(|g| value_number(&groups, &[&["data", g, "ratio"]]))
        } else {
            None
        }
    } else {
        None
    };
    Ok(AccountSummary {
        provider_id: provider.id,
        provider_name: provider.name,
        provider_type: provider.provider_type.clone(),
        supported: true,
        account_name: value_string(data, &[&["display_name"], &["username"], &["email"]]),
        balance: if matches!(provider.provider_type, ProviderType::NewApi) {
            value_number(data, &[&["quota"]]).map(|v| v / divisor)
        } else {
            value_number(data, &[&["balance"]])
        },
        used: if matches!(provider.provider_type, ProviderType::NewApi) {
            value_number(data, &[&["used_quota"]]).map(|v| v / divisor)
        } else {
            None
        },
        frozen_balance: value_number(data, &[&["frozen_balance"]]),
        request_count: value_number(data, &[&["request_count"]]),
        group,
        multiplier,
        subscription_name: first_sub.and_then(|v| {
            value_string(
                v,
                &[
                    &["plan", "name"],
                    &["group", "name"],
                    &["group_name"],
                    &["name"],
                ],
            )
        }),
        subscription_status: first_sub.and_then(|v| value_string(v, &[&["status"]])),
        expires_at: first_sub.and_then(|v| value_string(v, &[&["expires_at"], &["end_at"]])),
        token_expires_at: provider.token_expires_at,
        error: None,
        checked_at: Utc::now().to_rfc3339(),
    })
}

#[tauri::command]
async fn list_groups(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ManagedGroup>, String> {
    let provider = get_provider(&state, &provider_id)?;
    let client = management_client()?;
    let path = match provider.provider_type {
        ProviderType::NewApi => "/api/user/self/groups",
        ProviderType::Sub2api => "/api/v1/groups/available",
        _ => return Ok(vec![]),
    };
    let body = management_request(&client, &provider, reqwest::Method::GET, path, None).await?;
    let data = body.get("data").unwrap_or(&body);
    let groups = if let Some(items) = data.as_array() {
        items
            .iter()
            .map(|g| ManagedGroup {
                provider_id: provider.id.clone(),
                id: value_string(g, &[&["id"]])
                    .or_else(|| value_number(g, &[&["id"]]).map(|v| (v as i64).to_string()))
                    .unwrap_or_else(|| value_string(g, &[&["name"]]).unwrap_or_default()),
                name: value_string(g, &[&["name"]]).unwrap_or_default(),
                description: value_string(g, &[&["description"]]),
                multiplier: value_number(g, &[&["rate_multiplier"]]),
                subscription: value_string(g, &[&["subscription_type"]]).as_deref()
                    == Some("subscription"),
                available: true,
            })
            .collect()
    } else if let Some(map) = data.as_object() {
        map.iter()
            .map(|(name, g)| ManagedGroup {
                provider_id: provider.id.clone(),
                id: name.clone(),
                name: name.clone(),
                description: value_string(g, &[&["desc"]]),
                multiplier: value_number(g, &[&["ratio"]]),
                subscription: false,
                available: true,
            })
            .collect()
    } else {
        vec![]
    };
    Ok(groups)
}

fn timestamp_string(value: Option<f64>) -> Option<String> {
    value
        .filter(|v| *v > 0.0)
        .and_then(|v| chrono::DateTime::from_timestamp(v as i64, 0))
        .map(|v| v.to_rfc3339())
}

#[tauri::command]
async fn list_managed_keys(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<Vec<ManagedKey>, String> {
    let provider = get_provider(&state, &provider_id)?;
    let client = management_client()?;
    let path = match provider.provider_type {
        ProviderType::NewApi => "/api/token/?p=1&page_size=100",
        ProviderType::Sub2api => "/api/v1/keys?page=1&page_size=100",
        _ => return Ok(vec![]),
    };
    let body = management_request(&client, &provider, reqwest::Method::GET, path, None).await?;
    let data = body.get("data").unwrap_or(&body);
    let items = data
        .get("items")
        .and_then(Value::as_array)
        .or_else(|| data.as_array())
        .cloned()
        .unwrap_or_default();
    Ok(items
        .iter()
        .map(|k| {
            let sub = matches!(provider.provider_type, ProviderType::Sub2api);
            ManagedKey {
                provider_id: provider.id.clone(),
                provider_name: provider.name.clone(),
                id: value_number(k, &[&["id"]])
                    .map(|v| (v as i64).to_string())
                    .unwrap_or_default(),
                name: value_string(k, &[&["name"]]).unwrap_or_default(),
                masked_key: value_string(k, &[&["key"]]).unwrap_or("••••••••".into()),
                enabled: if sub {
                    value_string(k, &[&["status"]]).as_deref() == Some("active")
                } else {
                    value_number(k, &[&["status"]]).unwrap_or(1.0) == 1.0
                },
                group_id: if sub {
                    value_number(k, &[&["group_id"]]).map(|v| (v as i64).to_string())
                } else {
                    value_string(k, &[&["group"]])
                },
                group_name: value_string(k, &[&["group", "name"], &["group"]]),
                quota: if sub {
                    value_number(k, &[&["quota"]])
                } else {
                    value_number(k, &[&["remain_quota"]]).map(|v| v / 500_000.0)
                },
                used: if sub {
                    value_number(k, &[&["quota_used"]])
                } else {
                    value_number(k, &[&["used_quota"]]).map(|v| v / 500_000.0)
                },
                unlimited: if sub {
                    value_number(k, &[&["quota"]]).unwrap_or(0.0) == 0.0
                } else {
                    k.get("unlimited_quota")
                        .and_then(Value::as_bool)
                        .unwrap_or(false)
                },
                expires_at: if sub {
                    value_string(k, &[&["expires_at"]])
                } else {
                    timestamp_string(value_number(k, &[&["expired_time"]]))
                },
                last_used_at: if sub {
                    value_string(k, &[&["last_used_at"]])
                } else {
                    timestamp_string(value_number(k, &[&["accessed_time"]]))
                },
            }
        })
        .collect())
}

fn iso_to_timestamp(value: Option<&str>) -> i64 {
    value
        .and_then(|v| chrono::DateTime::parse_from_rfc3339(v).ok())
        .map(|v| v.timestamp())
        .unwrap_or(-1)
}

#[tauri::command]
async fn save_managed_key(
    state: State<'_, AppState>,
    input: ManagedKeyInput,
) -> Result<(), String> {
    let provider = get_provider(&state, &input.provider_id)?;
    let client = management_client()?;
    let payload = match provider.provider_type {
        ProviderType::NewApi => {
            json!({"id": input.id.as_deref().and_then(|v| v.parse::<i64>().ok()).unwrap_or(0), "name": input.name, "status": if input.enabled {1} else {2}, "group": input.group_id.unwrap_or_default(), "remain_quota": if input.unlimited {0} else {(input.quota.unwrap_or(0.0) * 500_000.0) as i64}, "unlimited_quota": input.unlimited, "expired_time": iso_to_timestamp(input.expires_at.as_deref()), "model_limits_enabled": false, "model_limits": "", "allow_ips": "", "cross_group_retry": false})
        }
        ProviderType::Sub2api => {
            json!({"name": input.name, "status": if input.enabled {"active"} else {"inactive"}, "group_id": input.group_id.and_then(|v| v.parse::<i64>().ok()), "quota": if input.unlimited {0.0} else {input.quota.unwrap_or(0.0)}, "expires_at": input.expires_at})
        }
        _ => return Err("该服务商不支持密钥管理".into()),
    };
    let (method, path) = match (&provider.provider_type, &input.id) {
        (ProviderType::NewApi, None) => (reqwest::Method::POST, "/api/token/".into()),
        (ProviderType::NewApi, Some(_)) => (reqwest::Method::PUT, "/api/token/".into()),
        (ProviderType::Sub2api, None) => (reqwest::Method::POST, "/api/v1/keys".into()),
        (ProviderType::Sub2api, Some(id)) => (reqwest::Method::PUT, format!("/api/v1/keys/{id}")),
        _ => return Err("该服务商不支持密钥管理".into()),
    };
    management_request(&client, &provider, method, &path, Some(payload)).await?;
    Ok(())
}

#[tauri::command]
async fn toggle_managed_key(
    state: State<'_, AppState>,
    provider_id: String,
    key_id: String,
    enabled: bool,
) -> Result<(), String> {
    let provider = get_provider(&state, &provider_id)?;
    let client = management_client()?;
    let (path, payload) = match provider.provider_type {
        ProviderType::NewApi => (
            "/api/token/?status_only=true".into(),
            json!({"id": key_id.parse::<i64>().unwrap_or(0), "status": if enabled {1} else {2}}),
        ),
        ProviderType::Sub2api => (
            format!("/api/v1/keys/{key_id}"),
            json!({"status": if enabled {"active"} else {"inactive"}}),
        ),
        _ => return Err("该服务商不支持密钥管理".into()),
    };
    management_request(
        &client,
        &provider,
        reqwest::Method::PUT,
        &path,
        Some(payload),
    )
    .await?;
    Ok(())
}

#[tauri::command]
async fn delete_managed_key(
    state: State<'_, AppState>,
    provider_id: String,
    key_id: String,
) -> Result<(), String> {
    let provider = get_provider(&state, &provider_id)?;
    let client = management_client()?;
    let path = match provider.provider_type {
        ProviderType::NewApi => format!("/api/token/{key_id}"),
        ProviderType::Sub2api => format!("/api/v1/keys/{key_id}"),
        _ => return Err("该服务商不支持密钥管理".into()),
    };
    management_request(&client, &provider, reqwest::Method::DELETE, &path, None).await?;
    Ok(())
}

#[tauri::command]
async fn load_usage(
    state: State<'_, AppState>,
    provider_id: String,
) -> Result<UsageSummary, String> {
    let provider = get_provider(&state, &provider_id)?;
    let client = management_client()?;
    let path = match provider.provider_type {
        ProviderType::NewApi => "/api/log/self/stat",
        ProviderType::Sub2api => "/api/v1/usage/dashboard/stats",
        _ => return Err("该服务商不支持统一统计".into()),
    };
    let body = management_request(&client, &provider, reqwest::Method::GET, path, None).await?;
    let data = body.get("data").unwrap_or(&body);
    Ok(UsageSummary {
        provider_id: provider.id,
        provider_name: provider.name,
        total_requests: value_number(data, &[&["total_requests"], &["request_count"]])
            .unwrap_or(0.0),
        total_tokens: value_number(data, &[&["total_tokens"]]),
        total_cost: value_number(data, &[&["total_actual_cost"], &["quota"]]).map(|v| {
            if matches!(provider.provider_type, ProviderType::NewApi) {
                v / 500_000.0
            } else {
                v
            }
        }),
        today_requests: value_number(data, &[&["today_requests"]]),
        today_tokens: value_number(data, &[&["today_tokens"]]),
        today_cost: value_number(data, &[&["today_actual_cost"]]),
        checked_at: Utc::now().to_rfc3339(),
    })
}

fn profiles_root(app: &AppHandle) -> Result<PathBuf, String> {
    let root = app
        .path()
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("profiles");
    fs::create_dir_all(&root).map_err(|e| e.to_string())?;
    Ok(root)
}

fn resolve_executable(profile: &ToolProfile) -> Result<String, String> {
    if let Some(path) = profile
        .executable
        .as_deref()
        .filter(|value| !value.trim().is_empty())
    {
        if Path::new(path).is_file() {
            return Ok(path.to_string());
        }
        return Err(format!("找不到可执行文件：{path}"));
    }
    let name = if profile.tool == "codex" {
        "codex"
    } else {
        "claude"
    };
    if let Ok(output) = Command::new("/usr/bin/which").arg(name).output() {
        if output.status.success() {
            let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
            if !path.is_empty() {
                return Ok(path);
            }
        }
    }
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or("无法确定用户主目录")?;
    let mut candidates = vec![
        home.join(".local/bin").join(name),
        PathBuf::from("/opt/homebrew/bin").join(name),
        PathBuf::from("/usr/local/bin").join(name),
    ];
    if let Ok(entries) = fs::read_dir(home.join(".nvm/versions/node")) {
        let mut versions: Vec<PathBuf> = entries
            .filter_map(Result::ok)
            .map(|entry| entry.path().join("bin").join(name))
            .filter(|path| path.is_file())
            .collect();
        versions.sort();
        versions.reverse();
        candidates.splice(0..0, versions);
    }
    candidates
        .into_iter()
        .find(|path| path.is_file())
        .map(|path| path.display().to_string())
        .ok_or_else(|| format!("未找到 {name}。请先安装，或在档案中填写可执行文件完整路径"))
}

fn profile_preview(
    app: &AppHandle,
    data: &PersistedData,
    profile: &ToolProfile,
) -> Result<LaunchPreview, String> {
    let provider = data
        .providers
        .iter()
        .find(|p| p.id == profile.provider_id)
        .ok_or("档案引用的服务商不存在")?;
    if !provider.has_api_key {
        return Err("服务商没有模型 API Key".into());
    }
    let executable = resolve_executable(profile)?;
    let untouched = vec![
        "~/.codex".into(),
        "~/.claude".into(),
        "~/.zshrc / ~/.bashrc".into(),
    ];
    if profile.tool == "codex" {
        let home = profiles_root(app)?.join("codex").join(&profile.id);
        Ok(LaunchPreview {
            profile_id: profile.id.clone(),
            tool: profile.tool.clone(),
            executable: executable.clone(),
            isolated_home: Some(home.display().to_string()),
            environment: vec![
                "CODEX_HOME=<ModelDeck 独立目录>".into(),
                "OPENAI_API_KEY=<系统钥匙串>".into(),
                format!("OPENAI_BASE_URL={}/v1", provider.base_url),
            ],
            untouched_paths: untouched,
            command_preview: format!("CODEX_HOME=… OPENAI_API_KEY=•••• {executable}"),
        })
    } else {
        Ok(LaunchPreview {
            profile_id: profile.id.clone(),
            tool: profile.tool.clone(),
            executable: executable.clone(),
            isolated_home: None,
            environment: vec![
                "ANTHROPIC_AUTH_TOKEN=<系统钥匙串>".into(),
                format!("ANTHROPIC_BASE_URL={}", provider.base_url),
            ],
            untouched_paths: untouched,
            command_preview: format!("ANTHROPIC_AUTH_TOKEN=•••• {executable}"),
        })
    }
}

fn upsert_tool_profile(
    data: &mut PersistedData,
    input: ToolProfileInput,
) -> Result<ToolProfile, String> {
    if input.name.trim().is_empty() || !matches!(input.tool.as_str(), "codex" | "claude") {
        return Err("档案名称或工具类型无效".into());
    }
    if !data.providers.iter().any(|p| p.id == input.provider_id) {
        return Err("服务商不存在".into());
    }
    let now = Utc::now().to_rfc3339();
    let id = input.id.unwrap_or_else(|| Uuid::new_v4().to_string());
    let existing = data.profiles.iter().find(|p| p.id == id).cloned();
    let profile = ToolProfile {
        id: id.clone(),
        name: input.name.trim().into(),
        tool: input.tool,
        provider_id: input.provider_id,
        model: input.model.filter(|v| !v.trim().is_empty()),
        executable: input.executable.filter(|v| !v.trim().is_empty()),
        active: existing.as_ref().is_some_and(|p| p.active),
        created_at: existing
            .as_ref()
            .map(|p| p.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now,
    };
    if let Some(position) = data.profiles.iter().position(|p| p.id == id) {
        data.profiles[position] = profile.clone();
    } else {
        data.profiles.push(profile.clone());
    }
    Ok(profile)
}

#[tauri::command]
fn save_tool_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    input: ToolProfileInput,
) -> Result<ToolProfile, String> {
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    let profile = upsert_tool_profile(&mut data, input)?;
    save_data(&app, &data)?;
    Ok(profile)
}

#[tauri::command]
fn delete_tool_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    data.profiles.retain(|p| p.id != profile_id);
    save_data(&app, &data)
}

#[tauri::command]
fn preview_tool_launch(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<LaunchPreview, String> {
    let data = state.0.lock().map_err(|e| e.to_string())?;
    let profile = data
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .ok_or("档案不存在")?;
    profile_preview(&app, &data, profile)
}

#[tauri::command]
fn launch_tool_profile(
    app: AppHandle,
    state: State<'_, AppState>,
    profile_id: String,
) -> Result<(), String> {
    let mut data = state.0.lock().map_err(|e| e.to_string())?;
    let profile = data
        .profiles
        .iter()
        .find(|p| p.id == profile_id)
        .cloned()
        .ok_or("档案不存在")?;
    let provider = data
        .providers
        .iter()
        .find(|p| p.id == profile.provider_id)
        .cloned()
        .ok_or("服务商不存在")?;
    let preview = profile_preview(&app, &data, &profile)?;
    let key = get_model_key(&provider.id)?;
    let shell;
    if profile.tool == "codex" {
        let home = PathBuf::from(preview.isolated_home.clone().ok_or("缺少隔离目录")?);
        fs::create_dir_all(&home).map_err(|e| e.to_string())?;
        shell = format!("export CODEX_HOME={}; export OPENAI_API_KEY={}; export OPENAI_BASE_URL={}/v1; exec {}{}", shell_quote(&home.display().to_string()), shell_quote(&key), shell_quote(&provider.base_url), shell_quote(&preview.executable), profile.model.as_ref().map(|m| format!(" --model {}", shell_quote(m))).unwrap_or_default());
    } else {
        shell = format!(
            "export ANTHROPIC_AUTH_TOKEN={}; export ANTHROPIC_BASE_URL={}; exec {}{}",
            shell_quote(&key),
            shell_quote(&provider.base_url),
            shell_quote(&preview.executable),
            profile
                .model
                .as_ref()
                .map(|m| format!(" --model {}", shell_quote(m)))
                .unwrap_or_default()
        );
    }
    let script = format!(
        "tell application \"Terminal\"\nactivate\ndo script {}\nend tell",
        apple_script_quote(&shell)
    );
    Command::new("/usr/bin/osascript")
        .arg("-e")
        .arg(script)
        .spawn()
        .map_err(|e| e.to_string())?;
    for item in &mut data.profiles {
        item.active = item.id == profile_id;
    }
    save_data(&app, &data)
}

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}
fn apple_script_quote(value: &str) -> String {
    format!("\"{}\"", value.replace('\\', "\\\\").replace('\"', "\\\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_nested_numeric_values_from_numbers_and_strings() {
        let value = json!({"data": {"quota": 72757, "ratio": "0.8"}});
        assert_eq!(value_number(&value, &[&["data", "quota"]]), Some(72757.0));
        assert_eq!(value_number(&value, &[&["data", "ratio"]]), Some(0.8));
    }

    #[test]
    fn converts_new_api_quota_to_usd() {
        let quota = 72757.0;
        let quota_per_unit = 500000.0;
        assert!((quota / quota_per_unit - 0.145514_f64).abs() < 0.000001);
    }

    #[test]
    fn reads_new_api_group_map() {
        let groups =
            json!({"data": {"default": {"ratio": 1.0, "desc": "默认"}, "vip": {"ratio": 0.8}}});
        assert_eq!(
            value_number(&groups, &[&["data", "vip", "ratio"]]),
            Some(0.8)
        );
        assert_eq!(
            value_string(&groups, &[&["data", "default", "desc"]]),
            Some("默认".into())
        );
    }

    #[test]
    fn reads_sub2api_profile_fields() {
        let profile = json!({"code": 0, "data": {"balance": 12.5, "frozen_balance": 1.25, "username": "demo"}});
        assert_eq!(value_number(&profile, &[&["data", "balance"]]), Some(12.5));
        assert_eq!(
            value_number(&profile, &[&["data", "frozen_balance"]]),
            Some(1.25)
        );
        assert_eq!(
            value_string(&profile, &[&["data", "username"]]),
            Some("demo".into())
        );
    }

    #[test]
    fn quotes_shell_and_apple_script_values() {
        assert_eq!(shell_quote("a'b"), "'a'\\''b'");
        assert_eq!(apple_script_quote("a\\b\"c"), "\"a\\\\b\\\"c\"");
    }

    #[test]
    fn finds_installed_tools_outside_minimal_app_path() {
        let profile = ToolProfile {
            id: "x".into(),
            name: "Codex".into(),
            tool: "codex".into(),
            provider_id: "p".into(),
            model: None,
            executable: None,
            active: false,
            created_at: "now".into(),
            updated_at: "now".into(),
        };
        let path = resolve_executable(&profile).unwrap();
        assert!(path.ends_with("/codex"));
        assert!(Path::new(&path).is_file());
    }

    #[test]
    fn saves_and_reloads_profile_from_legacy_data() {
        let mut data: PersistedData = serde_json::from_value(json!({
            "providers": [{"id":"p","name":"Demo","type":"new-api","baseUrl":"https://example.com","enabled":true,"createdAt":"now","updatedAt":"now","hasApiKey":true}],
            "models": [], "balances": []
        })).unwrap();
        let profile = upsert_tool_profile(
            &mut data,
            ToolProfileInput {
                id: None,
                name: "Work".into(),
                tool: "codex".into(),
                provider_id: "p".into(),
                model: Some("gpt-5".into()),
                executable: None,
            },
        )
        .unwrap();
        let serialized = serde_json::to_string(&data).unwrap();
        let loaded: PersistedData = serde_json::from_str(&serialized).unwrap();
        assert_eq!(loaded.profiles.len(), 1);
        assert_eq!(loaded.profiles[0].id, profile.id);
        assert_eq!(loaded.profiles[0].model.as_deref(), Some("gpt-5"));
    }

    #[test]
    fn loads_legacy_data_without_profiles() {
        let data: PersistedData =
            serde_json::from_value(json!({"providers": [], "models": [], "balances": []})).unwrap();
        assert!(data.profiles.is_empty());
    }

    #[test]
    fn normalizes_base_url_with_v1_suffix() {
        assert_eq!(
            normalize_url("https://example.com/v1/"),
            "https://example.com"
        );
        assert_eq!(normalize_url("https://example.com"), "https://example.com");
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            app.manage(AppState(Mutex::new(load_data(app.handle()))));
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_providers,
            get_snapshot,
            save_provider,
            delete_provider,
            toggle_provider,
            fetch_models,
            test_model,
            query_balance,
            storage_info,
            export_pi_models,
            sync_account,
            list_groups,
            list_managed_keys,
            save_managed_key,
            toggle_managed_key,
            delete_managed_key,
            load_usage,
            save_tool_profile,
            delete_tool_profile,
            preview_tool_launch,
            launch_tool_profile
        ])
        .run(tauri::generate_context!())
        .expect("error while running ModelDeck");
}
