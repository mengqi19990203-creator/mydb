use super::connections::{
    connection_target, ensure_writable, mask_mongo_uri, password_for,
};
use super::AppState;
use crate::models::{
    MongoCollectionInfo, MongoDocumentPage, MongoIndexInfo,
};
use futures_util::io::{AsyncReadExt, AsyncWriteExt};
use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
use mongodb::options::GridFsBucketOptions;
use mongodb::options::{ClientOptions, IndexOptions};
use mongodb::{Client, Collection, IndexModel};
use percent_encoding::{utf8_percent_encode, NON_ALPHANUMERIC};
use serde_json::Value;
use std::time::{Duration, Instant};
use tauri::{AppHandle, State};

const MAX_PAGE_SIZE: u64 = 1000;

fn encode(value: &str) -> String {
    utf8_percent_encode(value, NON_ALPHANUMERIC).to_string()
}

fn host_for_uri(host: &str) -> String {
    let host = host.trim();
    if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_string()
    }
}

fn mongo_uri(
    config: &crate::models::ConnectionConfig,
    host: &str,
    port: u16,
    password: Option<&str>,
    uri_secret: Option<&str>,
) -> Result<String, String> {
    if config.mongo_uri_mode {
        let uri = uri_secret
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .or_else(|| {
                let value = config.mongo_uri.trim();
                (!value.is_empty()).then_some(value)
            })
            .ok_or_else(|| "连接串不能为空".to_string())?;
        if uri.contains("******") {
            return Err("连接串密码不可用，请编辑连接并重新输入完整连接串".to_string());
        }
        if uri.chars().count() > 2048 {
            return Err("连接串不能超过 2048 个字符".to_string());
        }
        return Ok(uri.to_string());
    }

    let mut uri = String::from("mongodb://");
    if !config.username.is_empty() {
        uri.push_str(&encode(&config.username));
        if let Some(password) = password {
            uri.push(':');
            uri.push_str(&encode(password));
        }
        uri.push('@');
    }
    uri.push_str(&host_for_uri(host));
    uri.push(':');
    uri.push_str(&port.to_string());
    uri.push('/');
    if !config.database.trim().is_empty() {
        uri.push_str(&encode(config.database.trim()));
    }
    let mut params = vec!["appName=mydb".to_string()];
    let auth_source = if !config.mongo_auth_source.trim().is_empty() {
        config.mongo_auth_source.trim()
    } else {
        "admin"
    };
    if !config.username.is_empty() {
        params.push(format!("authSource={}", encode(auth_source)));
    }
    if !matches!(config.ssl_mode.as_str(), "" | "disabled") {
        params.push("tls=true".to_string());
        if !config.ssl_ca_path.is_empty() {
            params.push(format!("tlsCAFile={}", encode(&config.ssl_ca_path)));
        }
    }
    // A direct connection keeps a single SSH-tunnel seed usable; without it the
    // driver may try to discover replica-set members that are only reachable
    // from the remote network.
    if config.ssh_enabled {
        params.push("directConnection=true".to_string());
    }
    uri.push('?');
    uri.push_str(&params.join("&"));
    Ok(uri)
}

fn mongo_uri_password(uri: &str) -> Option<String> {
    let rest = uri.split_once("://")?.1;
    let authority_end = rest
        .find(|ch| matches!(ch, '/' | '?' | '#'))
        .unwrap_or(rest.len());
    let authority = &rest[..authority_end];
    let at = authority.rfind('@')?;
    let userinfo = &authority[..at];
    let (_, password) = userinfo.split_once(':')?;
    Some(password.to_string())
}

fn mongo_uri_query_secrets(uri: &str) -> Vec<String> {
    let Some((_, tail)) = uri.split_once('?') else {
        return Vec::new();
    };
    let query = tail.split('#').next().unwrap_or(tail);
    query
        .split('&')
        .filter_map(|pair| {
            let (key, value) = pair.split_once('=')?;
            let lower = key.to_ascii_lowercase();
            let sensitive = ["password", "secret", "token", "key"]
                .iter()
                .any(|needle| lower.contains(needle));
            (sensitive && !value.is_empty()).then_some(value.to_string())
        })
        .collect()
}

fn redact_mongo_error(message: &str, full_uri: &str, masked_uri: &str) -> String {
    let mut text = message.replace(full_uri, masked_uri);
    let mut secrets = mongo_uri_query_secrets(full_uri);
    if let Some(password) = mongo_uri_password(full_uri) {
        secrets.push(password);
    }
    for secret in secrets {
        if secret.is_empty() {
            continue;
        }
        text = text.replace(&secret, "******");
        if let Ok(decoded) = percent_encoding::percent_decode_str(&secret).decode_utf8() {
            let decoded = decoded.to_string();
            if !decoded.is_empty() {
                text = text.replace(&decoded, "******");
            }
        }
    }
    text
}

fn friendly_mongo_hint(message: &str) -> Option<&'static str> {
    let lower = message.to_ascii_lowercase();
    if lower.contains("authentication") || lower.contains("auth failed") {
        Some("认证失败，请检查用户名 / 密码 / authSource")
    } else if lower.contains("dns") || lower.contains("srv") || lower.contains("txt record") {
        Some("DNS SRV / TXT 记录不可用，请检查域名、网络或 DNS")
    } else if lower.contains("certificate") || lower.contains("tls") || lower.contains("ssl") {
        Some("TLS 失败，请检查证书与主机名")
    } else if lower.contains("timeout") || lower.contains("timed out") {
        Some("连接超时，请检查主机、端口、网络或 IP allowlist")
    } else {
        None
    }
}

fn mongo_error(prefix: &str, error: &str, full_uri: &str, masked_uri: &str) -> String {
    let message = redact_mongo_error(error, full_uri, masked_uri);
    match friendly_mongo_hint(&message) {
        Some(hint) => format!("{prefix}：{hint}（{message}）"),
        None if prefix.contains("解析") => format!(
            "{prefix}：{message}；密码中的特殊字符需要 percent-encode，或改用主机 / 端口模式"
        ),
        None => format!("{prefix}：{message}"),
    }
}

pub(crate) async fn connect_with_config(
    config: &crate::models::ConnectionConfig,
    host: &str,
    port: u16,
    password: Option<&str>,
    uri_secret: Option<&str>,
) -> Result<Client, String> {
    let uri = mongo_uri(config, host, port, password, uri_secret)?;
    let masked_uri = if config.mongo_uri_mode {
        mask_mongo_uri(&uri)
    } else {
        String::new()
    };
    let mut options = ClientOptions::parse(&uri)
        .await
        .map_err(|error| mongo_error("MongoDB 连接串无法解析", &error.to_string(), &uri, &masked_uri))?;
    if config.mongo_uri_mode {
        if options.app_name.is_none() {
            options.app_name = Some("mydb".to_string());
        }
    } else {
        options.app_name = Some("mydb".to_string());
    }
    options.connect_timeout = Some(Duration::from_secs(10));
    options.server_selection_timeout = Some(Duration::from_secs(10));
    let client = Client::with_options(options)
        .map_err(|error| mongo_error("创建 MongoDB 客户端失败", &error.to_string(), &uri, &masked_uri))?;
    tokio::time::timeout(Duration::from_secs(10), async {
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| mongo_error("MongoDB 连接失败", &error.to_string(), &uri, &masked_uri))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "MongoDB 连接超时（10 秒）".to_string())??;
    Ok(client)
}

pub(crate) async fn test_mongo_connection(
    config: &crate::models::ConnectionConfig,
    host: &str,
    port: u16,
    password: Option<&str>,
    uri_secret: Option<&str>,
) -> Result<(), String> {
    connect_with_config(config, host, port, password, uri_secret)
        .await
        .map(|_| ())
}

pub(crate) async fn client_for(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
) -> Result<Client, String> {
    if let Some(client) = state.mongo_clients.lock().await.get(connection_id).cloned() {
        return Ok(client);
    }
    let _connect_guard = state.mongo_connect_lock.lock().await;
    if let Some(client) = state.mongo_clients.lock().await.get(connection_id).cloned() {
        return Ok(client);
    }

    let connections = crate::storage::load_connections(app)?;
    let config = connections
        .into_iter()
        .find(|item| item.id == connection_id)
        .ok_or_else(|| format!("连接不存在: {connection_id}"))?;
    if config.driver != "mongodb" {
        return Err("当前连接不是 MongoDB".to_string());
    }

    if config.mongo_uri_mode {
        let secret = password_for(state, &config, "mongo-uri").await?;
        let client = connect_with_config(
            &config,
            &config.host,
            config.port,
            None,
            secret.as_deref(),
        )
        .await?;
        state
            .mongo_clients
            .lock()
            .await
            .insert(connection_id.to_string(), client.clone());
        return Ok(client);
    }

    let password = password_for(state, &config, "db").await?;
    let (host, port, tunnel) = connection_target(state, &config, None, None).await?;
    let client = connect_with_config(&config, &host, port, password.as_deref(), None).await?;

    if let Some(tunnel) = tunnel {
        state.mongo_tunnels.lock().await.insert(connection_id.to_string(), tunnel);
    }
    state
        .mongo_clients
        .lock()
        .await
        .insert(connection_id.to_string(), client.clone());
    Ok(client)
}

pub(crate) async fn ping(app: &AppHandle, state: &AppState, connection_id: &str) -> Result<(), String> {
    let client = client_for(app, state, connection_id).await?;
    tokio::time::timeout(Duration::from_secs(5), async {
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| format!("MongoDB PING 失败：{error}"))?;
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "MongoDB 健康检查超时（5 秒）".to_string())?
}

pub(crate) async fn reconnect(
    app: &AppHandle,
    state: &AppState,
    connection_id: &str,
) -> Result<(), String> {
    state.mongo_clients.lock().await.remove(connection_id);
    state.mongo_tunnels.lock().await.remove(connection_id);
    client_for(app, state, connection_id).await?;
    Ok(())
}

pub(crate) fn parse_object(text: &str, label: &str) -> Result<Document, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Document::new());
    }
    serde_json::from_str::<Document>(trimmed)
        .map_err(|error| format!("{label} 必须是合法的 JSON 对象：{error}"))
}

pub(crate) fn document_to_json(document: &Document) -> Result<Value, String> {
    serde_json::to_value(document)
        .map_err(|error| format!("MongoDB 文档转换为 JSON 失败：{error}"))
}

pub(crate) fn json_to_bson(value: &Value) -> Result<Bson, String> {
    serde_json::from_value::<Bson>(value.clone())
        .map_err(|error| format!("MongoDB _id 格式无效：{error}"))
}

#[tauri::command]
pub async fn mongo_list_databases(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
) -> Result<Vec<String>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let mut names = client
        .list_database_names()
        .await
        .map_err(|error| format!("读取 MongoDB 数据库失败：{error}"))?;
    names.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    Ok(names)
}

#[tauri::command]
pub async fn mongo_list_collections(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<Vec<MongoCollectionInfo>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let collections = client
        .database(&database)
        .list_collections()
        .await
        .map_err(|error| format!("读取 MongoDB 集合失败：{error}"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| format!("读取 MongoDB 集合失败：{error}"))?;
    let mut result: Vec<MongoCollectionInfo> = collections
        .into_iter()
        .map(|item| MongoCollectionInfo {
            name: item.name,
            kind: match item.collection_type {
                mongodb::results::CollectionType::View => "view",
                mongodb::results::CollectionType::Timeseries => "timeseries",
                _ => "collection",
            }
            .to_string(),
        })
        .collect();
    result.sort_by(|left, right| {
        left.name
            .to_lowercase()
            .cmp(&right.name.to_lowercase())
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(result)
}

#[tauri::command]
pub async fn mongo_find_documents(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    filter: String,
    projection: String,
    sort: String,
    skip: u64,
    limit: u64,
) -> Result<MongoDocumentPage, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let collection_handle: Collection<Document> =
        client.database(&database).collection(&collection);
    let filter_doc = parse_object(&filter, "筛选条件")?;
    let projection_doc = parse_object(&projection, "字段投影")?;
    let sort_doc = parse_object(&sort, "排序条件")?;
    let limit = limit.clamp(1, MAX_PAGE_SIZE);
    let started = Instant::now();
    let mut find = collection_handle.find(filter_doc.clone());
    if !projection_doc.is_empty() {
        find = find.projection(projection_doc);
    }
    if sort_doc.is_empty() {
        find = find.sort(doc! { "_id": 1 });
    } else {
        find = find.sort(sort_doc);
    }
    let find = find.skip(skip).limit(limit as i64);
    let (documents, total) = tokio::time::timeout(Duration::from_secs(30), async {
        let documents = find
            .await
            .map_err(|error| format!("查询 MongoDB 文档失败：{error}"))?
            .try_collect::<Vec<Document>>()
            .await
            .map_err(|error| format!("读取 MongoDB 游标失败：{error}"))?;
        let total = if filter_doc.is_empty() && skip == 0 {
            collection_handle
                .estimated_document_count()
                .await
                .map_err(|error| format!("统计 MongoDB 文档失败：{error}"))?
        } else {
            collection_handle
                .count_documents(filter_doc)
                .await
                .map_err(|error| format!("统计 MongoDB 文档失败：{error}"))?
        };
        Ok::<_, String>((documents, total))
    })
    .await
    .map_err(|_| "MongoDB 查询超时（30 秒）".to_string())??;
    let documents = documents
        .iter()
        .map(document_to_json)
        .collect::<Result<Vec<_>, _>>()?;
    let loaded = documents.len() as u64;
    Ok(MongoDocumentPage {
        documents,
        total,
        skip,
        limit,
        elapsed_ms: started.elapsed().as_millis() as u64,
        truncated: skip.saturating_add(loaded) < total,
    })
}

pub(crate) fn parse_pipeline(text: &str) -> Result<Vec<Document>, String> {
    let value: Value = serde_json::from_str(text.trim())
        .map_err(|error| format!("聚合管道必须是合法的 JSON 数组：{error}"))?;
    let stages = value
        .as_array()
        .ok_or_else(|| "聚合管道必须是 JSON 数组".to_string())?;
    for stage in stages {
        let Some(object) = stage.as_object() else {
            return Err("聚合管道的每个阶段都必须是对象".to_string());
        };
        if object.contains_key("$out") || object.contains_key("$merge") {
            return Err("暂不支持 $out / $merge 聚合阶段".to_string());
        }
    }
    serde_json::from_value::<Vec<Document>>(value)
        .map_err(|error| format!("聚合管道格式无效：{error}"))
}

#[tauri::command]
pub async fn mongo_aggregate_documents(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    pipeline: String,
    limit: u64,
) -> Result<MongoDocumentPage, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let pipeline = parse_pipeline(&pipeline)?;
    let limit = limit.clamp(1, MAX_PAGE_SIZE);
    let started = Instant::now();
    let mut cursor = client
        .database(&database)
        .collection::<Document>(&collection)
        .aggregate(pipeline)
        .await
        .map_err(|error| format!("执行 MongoDB 聚合失败：{error}"))?;
    let mut documents = Vec::new();
    tokio::time::timeout(Duration::from_secs(30), async {
        use futures_util::TryStreamExt;
        while let Some(document) = cursor
            .try_next()
            .await
            .map_err(|error| format!("读取 MongoDB 聚合结果失败：{error}"))?
        {
            documents.push(document);
            if documents.len() as u64 >= limit {
                break;
            }
        }
        Ok::<(), String>(())
    })
    .await
    .map_err(|_| "MongoDB 聚合超时（30 秒）".to_string())??;
    let loaded = documents.len() as u64;
    let documents = documents
        .iter()
        .map(document_to_json)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(MongoDocumentPage {
        documents,
        total: loaded,
        skip: 0,
        limit,
        elapsed_ms: started.elapsed().as_millis() as u64,
        truncated: loaded >= limit,
    })
}

#[tauri::command]
pub async fn mongo_insert_documents(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    documents: String,
) -> Result<u64, String> {
    ensure_writable(&app, &connection_id)?;
    let documents = serde_json::from_str::<Vec<Document>>(&documents)
        .map_err(|error| format!("批量导入内容必须是 JSON 数组：{error}"))?;
    if documents.is_empty() {
        return Err("没有可导入的文档".to_string());
    }
    if documents.len() > 10_000 {
        return Err("单次最多导入 10,000 个文档".to_string());
    }
    let count = documents.len() as u64;
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(&database)
        .collection::<Document>(&collection)
        .insert_many(documents)
        .await
        .map_err(|error| format!("批量插入 MongoDB 文档失败：{error}"))?;
    Ok(count)
}

#[tauri::command]
pub async fn mongo_insert_document(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    document: String,
) -> Result<Value, String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    let document = parse_object(&document, "文档")?;
    let result = client
        .database(&database)
        .collection::<Document>(&collection)
        .insert_one(document)
        .await
        .map_err(|error| format!("插入 MongoDB 文档失败：{error}"))?;
    document_to_json(&doc! { "_id": result.inserted_id })
}

#[tauri::command]
pub async fn mongo_replace_document(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    id: Value,
    document: String,
) -> Result<u64, String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    let id = json_to_bson(&id)?;
    let mut document = parse_object(&document, "文档")?;
    document.insert("_id", id.clone());
    let result = client
        .database(&database)
        .collection::<Document>(&collection)
        .replace_one(doc! { "_id": id }, document)
        .await
        .map_err(|error| format!("替换 MongoDB 文档失败：{error}"))?;
    Ok(result.modified_count)
}

#[tauri::command]
pub async fn mongo_update_document_fields(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    id: Value,
    fields: Value,
) -> Result<bool, String> {
    ensure_writable(&app, &connection_id)?;
    let object = fields
        .as_object()
        .ok_or_else(|| "待更新字段必须是 JSON 对象".to_string())?;
    if object.is_empty() {
        return Err("没有待更新的字段".to_string());
    }
    let mut set = Document::new();
    for (field, value) in object {
        if field.is_empty() || field.starts_with('$') || field.contains('.') {
            return Err(format!("网格编辑只支持顶层字段名：{field}"));
        }
        set.insert(field.clone(), json_to_bson(value)?);
    }
    let client = client_for(&app, &state, &connection_id).await?;
    let result = client
        .database(&database)
        .collection::<Document>(&collection)
        .update_one(doc! { "_id": json_to_bson(&id)? }, doc! { "$set": set })
        .await
        .map_err(|error| format!("更新 MongoDB 文档失败：{error}"))?;
    Ok(result.matched_count > 0)
}

#[tauri::command]
pub async fn mongo_delete_document(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    id: Value,
) -> Result<u64, String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    let id = json_to_bson(&id)?;
    let result = client
        .database(&database)
        .collection::<Document>(&collection)
        .delete_one(doc! { "_id": id })
        .await
        .map_err(|error| format!("删除 MongoDB 文档失败：{error}"))?;
    Ok(result.deleted_count)
}

#[tauri::command]
pub async fn mongo_list_indexes(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
) -> Result<Vec<MongoIndexInfo>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let indexes = client
        .database(&database)
        .collection::<Document>(&collection)
        .list_indexes()
        .await
        .map_err(|error| format!("读取 MongoDB 索引失败：{error}"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| format!("读取 MongoDB 索引失败：{error}"))?;
    indexes
        .into_iter()
        .map(|index| {
            let keys = document_to_json(&index.keys)?;
            Ok(MongoIndexInfo {
                name: index
                    .options
                    .as_ref()
                    .and_then(|options| options.name.clone())
                    .unwrap_or_else(|| "_id_".to_string()),
                keys,
                unique: index
                    .options
                    .as_ref()
                    .and_then(|options| options.unique)
                    .unwrap_or(false),
                sparse: index
                    .options
                    .as_ref()
                    .and_then(|options| options.sparse)
                    .unwrap_or(false),
                expire_after_seconds: index
                    .options
                    .as_ref()
                    .and_then(|options| options.expire_after)
                    .map(|duration| duration.as_secs() as i32),
            })
        })
        .collect()
}

#[tauri::command]
pub async fn mongo_create_index(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    keys: String,
    name: String,
    unique: bool,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    let keys = parse_object(&keys, "索引字段")?;
    if keys.is_empty() {
        return Err("索引字段不能为空".to_string());
    }
    let mut options = IndexOptions::default();
    if !name.trim().is_empty() {
        options.name = Some(name.trim().to_string());
    }
    options.unique = Some(unique);
    let index = IndexModel::builder()
        .keys(keys)
        .options(options)
        .build();
    client
        .database(&database)
        .collection::<Document>(&collection)
        .create_index(index)
        .await
        .map_err(|error| format!("创建 MongoDB 索引失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_drop_index(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
    name: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    if name == "_id_" {
        return Err("不能删除 MongoDB 的默认 _id 索引".to_string());
    }
    client
        .database(&database)
        .collection::<Document>(&collection)
        .drop_index(&name)
        .await
        .map_err(|error| format!("删除 MongoDB 索引失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_create_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    name: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    if name.trim().is_empty() {
        return Err("集合名称不能为空".to_string());
    }
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(&database)
        .create_collection(name.trim())
        .await
        .map_err(|error| format!("创建 MongoDB 集合失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_drop_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    name: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(&database)
        .collection::<Document>(&name)
        .drop()
        .await
        .map_err(|error| format!("删除 MongoDB 集合失败：{error}"))?;
    Ok(())
}

fn mongo_manage_error(action: &str, error: &str) -> String {
    let lower = error.to_ascii_lowercase();
    if lower.contains("not authorized")
        || lower.contains("unauthorized")
        || lower.contains("requires authentication")
        || lower.contains("permission")
    {
        format!("{action}：{error}（当前用户可能没有该操作权限）")
    } else {
        format!("{action}：{error}")
    }
}

fn mongo_database_name_error(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return Some("数据库名称不能为空".to_string());
    }
    if name.len() > 64 {
        return Some("数据库名称不能超过 64 个字符".to_string());
    }
    if name.chars().any(|ch| ch == '\0' || "/\\.\"$*<>:|?".contains(ch)) {
        return Some("数据库名称不能包含 / \\ . \" $ * < > : | ? 等字符".to_string());
    }
    if matches!(name, "admin" | "local" | "config") {
        return Some("不能新建或删除系统数据库 admin / local / config".to_string());
    }
    None
}

fn mongo_collection_name_error(name: &str) -> Option<String> {
    let name = name.trim();
    if name.is_empty() {
        return Some("集合名称不能为空".to_string());
    }
    if name.len() > 120 {
        return Some("集合名称不能超过 120 个字符".to_string());
    }
    if name.contains('\0') || name.contains('$') {
        return Some("集合名称不能包含空字符或 $".to_string());
    }
    if name.starts_with("system.") {
        return Some("不能创建或重命名为 system. 开头的系统集合".to_string());
    }
    None
}

fn bson_u64(value: Option<&Bson>) -> Option<u64> {
    match value? {
        Bson::Int32(value) if *value >= 0 => Some(*value as u64),
        Bson::Int64(value) if *value >= 0 => Some(*value as u64),
        Bson::Double(value) if *value >= 0.0 => Some(*value as u64),
        _ => None,
    }
}

fn bson_f64(value: Option<&Bson>) -> Option<f64> {
    match value? {
        Bson::Int32(value) => Some(*value as f64),
        Bson::Int64(value) => Some(*value as f64),
        Bson::Double(value) => Some(*value),
        _ => None,
    }
}

fn collection_kind_from_stats(stats: &Document) -> String {
    if stats.contains_key("timeseries") {
        "timeseries".to_string()
    } else {
        "collection".to_string()
    }
}

#[tauri::command]
pub async fn mongo_create_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    first_collection: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    if let Some(error) = mongo_database_name_error(&database) {
        return Err(error);
    }
    if let Some(error) = mongo_collection_name_error(&first_collection) {
        return Err(error);
    }
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(database.trim())
        .create_collection(first_collection.trim())
        .await
        .map_err(|error| mongo_manage_error("创建 MongoDB 数据库失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_drop_database(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    confirm_name: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    if let Some(error) = mongo_database_name_error(&database) {
        return Err(error);
    }
    if confirm_name.trim() != database.trim() {
        return Err("输入的数据库名称与当前数据库不一致".to_string());
    }
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(database.trim())
        .drop()
        .await
        .map_err(|error| mongo_manage_error("删除 MongoDB 数据库失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_rename_collection(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    from: String,
    to: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let database = database.trim();
    let from = from.trim();
    let to = to.trim();
    if database.is_empty() || from.is_empty() {
        return Err("数据库名与源集合名不能为空".to_string());
    }
    if from == to {
        return Err("新集合名不能与源集合名相同".to_string());
    }
    if let Some(error) = mongo_collection_name_error(to) {
        return Err(error);
    }
    let client = client_for(&app, &state, &connection_id).await?;
    let collections = client
        .database(database)
        .list_collections()
        .await
        .map_err(|error| mongo_manage_error("读取集合列表失败", &error.to_string()))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| mongo_manage_error("读取集合列表失败", &error.to_string()))?;
    let source = collections
        .iter()
        .find(|item| item.name == from)
        .ok_or_else(|| format!("集合 {database}.{from} 已不存在，请刷新对象列表"))?;
    if !matches!(source.collection_type, mongodb::results::CollectionType::Collection) {
        return Err("视图 / 时间序列集合暂不支持从此处重命名".to_string());
    }
    if collections.iter().any(|item| item.name == to) {
        return Err(format!("目标集合 {database}.{to} 已存在，请换一个名称"));
    }
    let command = doc! {
        "renameCollection": format!("{database}.{from}"),
        "to": format!("{database}.{to}"),
        "dropTarget": false,
    };
    tokio::time::timeout(
        Duration::from_secs(30),
        client.database("admin").run_command(command),
    )
    .await
    .map_err(|_| "重命名集合超时（30 秒）".to_string())?
    .map_err(|error| mongo_manage_error("重命名 MongoDB 集合失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_collection_stats(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    collection: String,
) -> Result<crate::models::MongoCollectionStats, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let stats = tokio::time::timeout(
        Duration::from_secs(10),
        client
            .database(database.trim())
            .run_command(doc! { "collStats": collection.trim() }),
    )
    .await
    .map_err(|_| "读取集合统计超时（10 秒）".to_string())?
    .map_err(|error| mongo_manage_error("读取集合统计失败", &error.to_string()))?;

    let mut index_sizes: Vec<crate::models::MongoIndexSize> = stats
        .get_document("indexSizes")
        .map(|sizes| {
            sizes
                .iter()
                .filter_map(|(name, value)| {
                    bson_u64(Some(value)).map(|size| crate::models::MongoIndexSize {
                        name: name.clone(),
                        size,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    index_sizes.sort_by(|left, right| {
        right
            .size
            .cmp(&left.size)
            .then_with(|| left.name.cmp(&right.name))
    });
    index_sizes.truncate(50);

    let timeseries = stats.get_document("timeseries").ok().map(|info| {
        crate::models::MongoTimeseriesInfo {
            time_field: info.get_str("timeField").ok().map(str::to_string),
            meta_field: info.get_str("metaField").ok().map(str::to_string),
            granularity: info.get_str("granularity").ok().map(str::to_string),
        }
    });
    let validator = stats
        .get_document("validator")
        .ok()
        .and_then(|value| document_to_json(value).ok());

    Ok(crate::models::MongoCollectionStats {
        namespace: stats
            .get_str("ns")
            .map(str::to_string)
            .unwrap_or_else(|_| format!("{}.{}", database.trim(), collection.trim())),
        kind: collection_kind_from_stats(&stats),
        capped: stats.get_bool("capped").unwrap_or(false),
        count: bson_u64(stats.get("count")),
        size: bson_u64(stats.get("size")),
        storage_size: bson_u64(stats.get("storageSize")),
        avg_obj_size: bson_f64(stats.get("avgObjSize")),
        nindexes: bson_u64(stats.get("nindexes")).map(|value| value as u32),
        total_index_size: bson_u64(stats.get("totalIndexSize")),
        index_sizes,
        timeseries,
        validator,
        read_only: stats.get_bool("readOnly").ok(),
    })
}

fn mongo_regex_escape(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        if matches!(character, '\\' | '^' | '$' | '.' | '|' | '?' | '*' | '+' | '(' | ')' | '[' | ']' | '{' | '}') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn gridfs_file_json(file: &mongodb::gridfs::FilesCollectionDocument) -> Result<crate::models::MongoGridFsFile, String> {
    let id = document_to_json(&doc! { "_id": file.id.clone() })?
        .get("_id")
        .cloned()
        .unwrap_or(Value::Null);
    Ok(crate::models::MongoGridFsFile {
        id,
        filename: file.filename.clone().unwrap_or_default(),
        length: file.length,
        chunk_size: file.chunk_size_bytes,
        upload_date: file
            .upload_date
            .try_to_rfc3339_string()
            .unwrap_or_else(|_| String::new()),
        metadata: file
            .metadata
            .as_ref()
            .map(document_to_json)
            .transpose()?,
    })
}

fn gridfs_bucket(
    client: &Client,
    database: &str,
    bucket: &str,
) -> mongodb::gridfs::GridFsBucket {
    client.database(database).gridfs_bucket(
        GridFsBucketOptions::builder()
            .bucket_name(Some(bucket.trim().to_string()))
            .build(),
    )
}

fn mongo_roles_array(roles: &[String], default_database: &str) -> Result<Vec<Bson>, String> {
    if roles.len() > 100 {
        return Err("一次最多设置 100 个角色".to_string());
    }
    let mut result = Vec::new();
    for raw in roles {
        let role = raw.trim();
        if role.is_empty() {
            continue;
        }
        let (name, database) = role
            .split_once('@')
            .map(|(name, database)| (name.trim(), database.trim()))
            .unwrap_or((role, default_database));
        if name.is_empty() || database.is_empty() || name.contains('\0') || database.contains('\0') {
            return Err(format!("角色格式无效：{raw}"));
        }
        result.push(Bson::Document(doc! { "role": name, "db": database }));
    }
    Ok(result)
}

#[tauri::command]
pub async fn mongo_list_users(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<Vec<crate::models::MongoUserInfo>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let result = client
        .database(database.trim())
        .run_command(doc! { "usersInfo": 1 })
        .await
        .map_err(|error| mongo_manage_error("读取 MongoDB 用户失败", &error.to_string()))?;
    let users = result
        .get_array("users")
        .map_err(|_| "MongoDB usersInfo 返回格式无效".to_string())?
        .clone();
    Ok(users
        .into_iter()
        .filter_map(|value| value.as_document().cloned())
        .filter_map(|user| {
            let username = user.get_str("user").ok()?.to_string();
            let db = user.get_str("db").unwrap_or(database.trim()).to_string();
            let roles = user
                .get_array("roles")
                .ok()
                .map(|roles| {
                    roles
                        .iter()
                        .filter_map(|role| role.as_document())
                        .filter_map(|role| {
                            let name = role.get_str("role").ok()?;
                            let role_db = role.get_str("db").ok()?;
                            Some(format!("{name}@{role_db}"))
                        })
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            Some(crate::models::MongoUserInfo {
                username,
                database: db,
                roles,
            })
        })
        .collect())
}

#[tauri::command]
pub async fn mongo_list_roles(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<Vec<crate::models::MongoRoleInfo>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let result = client
        .database(database.trim())
        .run_command(doc! { "rolesInfo": 1 })
        .await
        .map_err(|error| mongo_manage_error("读取 MongoDB 角色失败", &error.to_string()))?;
    let roles = result
        .get_array("roles")
        .map_err(|_| "MongoDB rolesInfo 返回格式无效".to_string())?
        .clone();
    let mut output: Vec<crate::models::MongoRoleInfo> = roles
        .into_iter()
        .filter_map(|value| value.as_document().cloned())
        .filter_map(|role| {
            Some(crate::models::MongoRoleInfo {
                name: role.get_str("role").ok()?.to_string(),
                database: role.get_str("db").ok()?.to_string(),
                builtin: role.get_bool("isBuiltin").unwrap_or(false),
            })
        })
        .collect();
    output.sort_by(|left, right| {
        left.database
            .cmp(&right.database)
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(output)
}

#[tauri::command]
pub async fn mongo_create_user(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    username: String,
    password: String,
    roles: Vec<String>,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    if password.is_empty() {
        return Err("密码不能为空".to_string());
    }
    let role_docs = mongo_roles_array(&roles, database.trim())?;
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(database.trim())
        .run_command(doc! {
            "createUser": username,
            "pwd": password,
            "roles": role_docs,
        })
        .await
        .map_err(|error| mongo_manage_error("创建 MongoDB 用户失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_update_user_roles(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    username: String,
    roles: Vec<String>,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    let role_docs = mongo_roles_array(&roles, database.trim())?;
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(database.trim())
        .run_command(doc! {
            "updateUser": username,
            "roles": role_docs,
        })
        .await
        .map_err(|error| mongo_manage_error("更新 MongoDB 用户角色失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_drop_user(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    username: String,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let username = username.trim();
    if username.is_empty() {
        return Err("用户名不能为空".to_string());
    }
    let client = client_for(&app, &state, &connection_id).await?;
    client
        .database(database.trim())
        .run_command(doc! { "dropUser": username })
        .await
        .map_err(|error| mongo_manage_error("删除 MongoDB 用户失败", &error.to_string()))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_list_gridfs_buckets(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
) -> Result<Vec<String>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let names = client
        .database(database.trim())
        .list_collection_names()
        .await
        .map_err(|error| format!("读取 GridFS bucket 失败：{error}"))?;
    let mut buckets: Vec<String> = names
        .into_iter()
        .filter_map(|name| name.strip_suffix(".files").map(str::to_string))
        .filter(|name| !name.is_empty())
        .collect();
    buckets.sort_by(|left, right| left.to_lowercase().cmp(&right.to_lowercase()));
    buckets.dedup();
    Ok(buckets)
}

#[tauri::command]
pub async fn mongo_list_gridfs_files(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    bucket: String,
    search: String,
    skip: u64,
    limit: u64,
) -> Result<Vec<crate::models::MongoGridFsFile>, String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let bucket = gridfs_bucket(&client, database.trim(), &bucket);
    let filter = if search.trim().is_empty() {
        Document::new()
    } else {
        doc! {
            "filename": {
                "$regex": mongo_regex_escape(search.trim()),
                "$options": "i"
            }
        }
    };
    let files = bucket
        .find(filter)
        .sort(doc! { "uploadDate": -1 })
        .skip(skip)
        .limit(limit.clamp(1, 1000) as i64)
        .await
        .map_err(|error| format!("读取 GridFS 文件失败：{error}"))?
        .try_collect::<Vec<_>>()
        .await
        .map_err(|error| format!("读取 GridFS 文件失败：{error}"))?;
    files.iter().map(gridfs_file_json).collect()
}

#[tauri::command]
pub async fn mongo_upload_gridfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    bucket: String,
    path: String,
    filename: Option<String>,
) -> Result<crate::models::MongoGridFsFile, String> {
    ensure_writable(&app, &connection_id)?;
    let metadata = tokio::fs::metadata(&path)
        .await
        .map_err(|error| format!("读取待上传文件失败：{error}"))?;
    if !metadata.is_file() {
        return Err("上传路径不是文件".to_string());
    }
    if metadata.len() > 200 * 1024 * 1024 {
        return Err("GridFS 上传第一版限制 200 MB".to_string());
    }
    let name = filename
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| {
            std::path::Path::new(&path)
                .file_name()
                .and_then(|value| value.to_str())
                .map(str::to_string)
        })
        .ok_or_else(|| "无法确定文件名".to_string())?;
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|error| format!("读取待上传文件失败：{error}"))?;
    let client = client_for(&app, &state, &connection_id).await?;
    let bucket = gridfs_bucket(&client, database.trim(), &bucket);
    let mut stream = bucket
        .open_upload_stream(name)
        .await
        .map_err(|error| format!("创建 GridFS 上传流失败：{error}"))?;
    stream
        .write_all(&bytes)
        .await
        .map_err(|error| format!("写入 GridFS 失败：{error}"))?;
    stream
        .close()
        .await
        .map_err(|error| format!("完成 GridFS 上传失败：{error}"))?;
    let id = stream.id().clone();
    let file = bucket
        .find(doc! { "_id": id })
        .await
        .map_err(|error| format!("读取上传结果失败：{error}"))?
        .try_next()
        .await
        .map_err(|error| format!("读取上传结果失败：{error}"))?
        .ok_or_else(|| "上传完成但找不到文件记录".to_string())?;
    gridfs_file_json(&file)
}

#[tauri::command]
pub async fn mongo_download_gridfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    bucket: String,
    id: Value,
    destination: String,
) -> Result<(), String> {
    let client = client_for(&app, &state, &connection_id).await?;
    let bucket = gridfs_bucket(&client, database.trim(), &bucket);
    let id = json_to_bson(&id)?;
    let mut stream = bucket
        .open_download_stream(id)
        .await
        .map_err(|error| format!("打开 GridFS 文件失败：{error}"))?;
    let mut bytes = Vec::new();
    stream
        .read_to_end(&mut bytes)
        .await
        .map_err(|error| format!("读取 GridFS 文件失败：{error}"))?;
    if bytes.len() > 500 * 1024 * 1024 {
        return Err("GridFS 下载第一版限制 500 MB".to_string());
    }
    tokio::fs::write(&destination, bytes)
        .await
        .map_err(|error| format!("写入下载文件失败：{error}"))?;
    Ok(())
}

#[tauri::command]
pub async fn mongo_delete_gridfs_file(
    app: AppHandle,
    state: State<'_, AppState>,
    connection_id: String,
    database: String,
    bucket: String,
    id: Value,
) -> Result<(), String> {
    ensure_writable(&app, &connection_id)?;
    let client = client_for(&app, &state, &connection_id).await?;
    let bucket = gridfs_bucket(&client, database.trim(), &bucket);
    let id = json_to_bson(&id)?;
    bucket
        .delete(id)
        .await
        .map_err(|error| format!("删除 GridFS 文件失败：{error}"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ConnectionConfig;

    #[test]
    fn builds_encoded_uri_with_authentication_and_tls() {
        let mut config = ConnectionConfig::default();
        config.username = "admin@corp".to_string();
        config.database = "app db".to_string();
        config.mongo_auth_source = "admin".to_string();
        config.ssl_mode = "required".to_string();
        let uri = mongo_uri(&config, "mongo.example.com", 27017, Some("p@ss:word"), None).unwrap();
        assert!(uri.starts_with("mongodb://admin%40corp:p%40ss%3Aword@mongo.example.com:27017/app%20db?"));
        assert!(uri.contains("authSource=admin"));
        assert!(uri.contains("tls=true"));
    }

    #[test]
    fn uses_full_mongodb_uri_in_uri_mode() {
        let mut config = ConnectionConfig::default();
        config.mongo_uri_mode = true;
        let uri = mongo_uri(
            &config,
            "ignored",
            27017,
            None,
            Some("mongodb+srv://user:pw@cluster0.example.net/app?retryWrites=true&w=majority"),
        )
        .unwrap();
        assert!(uri.starts_with("mongodb+srv://user:pw@cluster0.example.net/app"));
    }

    #[test]
    fn refuses_masked_uri_without_secret() {
        let mut config = ConnectionConfig::default();
        config.mongo_uri_mode = true;
        config.mongo_uri = "mongodb://user:******@host/app".to_string();
        assert!(mongo_uri(&config, "host", 27017, None, None).is_err());
    }

    #[test]
    fn redacts_uri_password_from_errors() {
        let uri = "mongodb://user:s3cret@host/app?authSource=admin";
        let masked = mask_mongo_uri(uri);
        let message = redact_mongo_error(
            "failed to connect mongodb://user:s3cret@host/app?authSource=admin as s3cret",
            uri,
            &masked,
        );
        assert!(!message.contains("s3cret"));
        assert!(message.contains("******"));

        let uri = "mongodb://host/app?token=abc123&authSource=admin";
        let message = redact_mongo_error("token abc123 rejected", uri, &mask_mongo_uri(uri));
        assert!(!message.contains("abc123"));
    }

    #[tokio::test]
    #[ignore = "requires MYDB_TEST_MONGO_URI"]
    async fn connects_with_real_mongodb_uri() {
        let uri = std::env::var("MYDB_TEST_MONGO_URI")
            .expect("set MYDB_TEST_MONGO_URI to run the URI regression");
        let mut config = ConnectionConfig::default();
        config.driver = "mongodb".to_string();
        config.mongo_uri_mode = true;
        config.mongo_uri = mask_mongo_uri(&uri);
        let client = connect_with_config(&config, "", 27017, None, Some(&uri))
            .await
            .expect("URI connection should succeed");
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .expect("ping should succeed");
    }

    #[test]
    fn validates_management_names_and_system_databases() {
        for name in ["admin", "local", "config", "bad/name", "bad.name", ""] {
            assert!(mongo_database_name_error(name).is_some(), "{name}");
        }
        assert!(mongo_database_name_error("mydb_i03").is_none());
        for name in ["", "bad$name", "system.users"] {
            assert!(mongo_collection_name_error(name).is_some(), "{name}");
        }
        assert!(mongo_collection_name_error("events_2026").is_none());
    }

    #[test]
    fn reads_stats_numbers_and_timeseries_kind() {
        let stats = doc! {
            "count": 12_i64,
            "size": 2048_i32,
            "avgObjSize": 170.5_f64,
            "timeseries": { "timeField": "ts", "granularity": "seconds" }
        };
        assert_eq!(bson_u64(stats.get("count")), Some(12));
        assert_eq!(bson_u64(stats.get("size")), Some(2048));
        assert_eq!(bson_f64(stats.get("avgObjSize")), Some(170.5));
        assert_eq!(collection_kind_from_stats(&stats), "timeseries");
    }

    #[tokio::test]
    #[ignore = "requires MYDB_TEST_MONGO_URI and creates a temporary database"]
    async fn manages_real_mongodb_database() {
        let uri = std::env::var("MYDB_TEST_MONGO_URI")
            .expect("set MYDB_TEST_MONGO_URI to run the management regression");
        let mut config = ConnectionConfig::default();
        config.driver = "mongodb".to_string();
        config.mongo_uri_mode = true;
        config.mongo_uri = mask_mongo_uri(&uri);
        let client = connect_with_config(&config, "", 27017, None, Some(&uri))
            .await
            .expect("URI connection should succeed");
        let database = format!(
            "mydb_i03_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let db = client.database(&database);
        db.create_collection("seed").await.expect("create collection");
        db.collection::<Document>("seed")
            .insert_one(doc! { "ok": true })
            .await
            .expect("insert seed");
        let stats = db
            .run_command(doc! { "collStats": "seed" })
            .await
            .expect("collStats");
        assert_eq!(bson_u64(stats.get("count")), Some(1));
        let command = doc! {
            "renameCollection": format!("{database}.seed"),
            "to": format!("{database}.seed_renamed"),
            "dropTarget": false,
        };
        client
            .database("admin")
            .run_command(command)
            .await
            .expect("rename collection");
        assert!(db
            .list_collection_names()
            .await
            .expect("list collections")
            .contains(&"seed_renamed".to_string()));
        db.drop().await.expect("drop temporary database");
    }

    #[tokio::test]
    #[ignore = "requires MYDB_TEST_MONGO_URI and creates a temporary database"]
    async fn manages_real_gridfs_file() {
        let uri = std::env::var("MYDB_TEST_MONGO_URI")
            .expect("set MYDB_TEST_MONGO_URI to run the GridFS regression");
        let mut config = ConnectionConfig::default();
        config.driver = "mongodb".to_string();
        config.mongo_uri_mode = true;
        config.mongo_uri = mask_mongo_uri(&uri);
        let client = connect_with_config(&config, "", 27017, None, Some(&uri))
            .await
            .expect("URI connection should succeed");
        let database = format!(
            "mydb_gridfs_{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        );
        let db = client.database(&database);
        let bucket = db.gridfs_bucket(
            GridFsBucketOptions::builder()
                .bucket_name(Some("test".to_string()))
                .build(),
        );
        let mut upload = bucket.open_upload_stream("hello.txt").await.unwrap();
        upload.write_all(b"hello gridfs").await.unwrap();
        upload.close().await.unwrap();
        let id = upload.id().clone();
        let file = bucket
            .find(doc! { "_id": id.clone() })
            .await
            .unwrap()
            .try_next()
            .await
            .unwrap()
            .expect("file record");
        assert_eq!(file.filename.as_deref(), Some("hello.txt"));
        let mut download = bucket.open_download_stream(id.clone()).await.unwrap();
        let mut bytes = Vec::new();
        download.read_to_end(&mut bytes).await.unwrap();
        assert_eq!(bytes, b"hello gridfs");
        bucket.delete(id).await.unwrap();
        db.drop().await.unwrap();
    }

    #[test]
    fn parses_extended_json_objects() {
        let document = parse_object(r#"{"_id":{"$oid":"507f1f77bcf86cd799439011"},"n":1}"#, "文档").unwrap();
        assert!(document.get("_id").is_some());
        assert_eq!(document.get_i32("n").unwrap(), 1);
    }

    #[test]
    fn converts_bson_ids_from_extended_json() {
        let id = serde_json::json!({"$oid": "507f1f77bcf86cd799439011"});
        assert!(json_to_bson(&id).is_ok());
    }

    #[test]
    fn serializes_object_ids_as_extended_json() {
        use mongodb::bson::oid::ObjectId;
        let id = ObjectId::parse_str("507f1f77bcf86cd799439011").unwrap();
        let value = document_to_json(&doc! { "_id": id }).unwrap();
        assert_eq!(
            value["_id"]["$oid"].as_str(),
            Some("507f1f77bcf86cd799439011")
        );
    }
    #[test]
    fn validates_aggregation_pipelines() {
        assert!(parse_pipeline(r#"[{ "$match": {} }, { "$limit": 1 }]"#).is_ok());
        assert!(parse_pipeline(r#"{ "$match": {} }"#).is_err());
        assert!(parse_pipeline(r#"[{ "$out": "archive" }]"#).is_err());
        assert!(parse_pipeline(r#"[{ "$merge": { "into": "archive" } }]"#).is_err());
    }

}
