use super::connections::{
    connection_target, ensure_writable, password_for,
};
use super::AppState;
use crate::models::{
    MongoCollectionInfo, MongoDocumentPage, MongoIndexInfo,
};
use futures_util::TryStreamExt;
use mongodb::bson::{doc, Bson, Document};
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

fn mongo_uri(config: &crate::models::ConnectionConfig, host: &str, port: u16, password: Option<&str>) -> String {
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
    uri
}

async fn connect_with_config(
    config: &crate::models::ConnectionConfig,
    host: &str,
    port: u16,
    password: Option<&str>,
) -> Result<Client, String> {
    let uri = mongo_uri(config, host, port, password);
    let mut options = ClientOptions::parse(&uri)
        .await
        .map_err(|error| format!("MongoDB 连接串无法解析：{error}"))?;
    options.app_name = Some("mydb".to_string());
    options.connect_timeout = Some(Duration::from_secs(10));
    options.server_selection_timeout = Some(Duration::from_secs(10));
    let client = Client::with_options(options)
        .map_err(|error| format!("创建 MongoDB 客户端失败：{error}"))?;
    tokio::time::timeout(Duration::from_secs(10), async {
        client
            .database("admin")
            .run_command(doc! { "ping": 1 })
            .await
            .map_err(|error| format!("MongoDB 连接失败：{error}"))?;
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
) -> Result<(), String> {
    connect_with_config(config, host, port, password)
        .await
        .map(|_| ())
}

async fn client_for(
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

    let password = password_for(state, &config, "db").await?;
    let (host, port, tunnel) = connection_target(state, &config, None, None).await?;
    let client = connect_with_config(&config, &host, port, password.as_deref()).await?;

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

fn parse_object(text: &str, label: &str) -> Result<Document, String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return Ok(Document::new());
    }
    serde_json::from_str::<Document>(trimmed)
        .map_err(|error| format!("{label} 必须是合法的 JSON 对象：{error}"))
}

fn document_to_json(document: &Document) -> Result<Value, String> {
    serde_json::to_value(document)
        .map_err(|error| format!("MongoDB 文档转换为 JSON 失败：{error}"))
}

fn json_to_bson(value: &Value) -> Result<Bson, String> {
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

fn parse_pipeline(text: &str) -> Result<Vec<Document>, String> {
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
        let uri = mongo_uri(&config, "mongo.example.com", 27017, Some("p@ss:word"));
        assert!(uri.starts_with("mongodb://admin%40corp:p%40ss%3Aword@mongo.example.com:27017/app%20db?"));
        assert!(uri.contains("authSource=admin"));
        assert!(uri.contains("tls=true"));
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
