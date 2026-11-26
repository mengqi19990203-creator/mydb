mod backup_store;
mod commands;
mod db;
mod history;
mod keychain;
mod models;
mod redis_client;
mod ssh;
mod storage;

use commands::AppState;
use tauri::Emitter;
use tauri::Manager;
use tauri::menu::{
    CheckMenuItemBuilder, MenuBuilder, MenuItemBuilder, MenuItemKind, PredefinedMenuItem,
    SubmenuBuilder,
};

#[cfg(target_os = "macos")]
fn build_menu(handle: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let settings = MenuItemBuilder::with_id("menu-settings", "设置…")
        .accelerator("CmdOrCtrl+,")
        .build(handle)?;
    let app_menu = SubmenuBuilder::new(handle, "mydb")
        .item(&PredefinedMenuItem::about(handle, Some("关于 mydb"), None)?)
        .item(&settings)
        .separator()
        .item(&PredefinedMenuItem::services(handle, Some("服务"))?)
        .separator()
        .item(&PredefinedMenuItem::hide(handle, Some("隐藏 mydb"))?)
        .item(&PredefinedMenuItem::hide_others(handle, Some("隐藏其他"))?)
        .item(&PredefinedMenuItem::show_all(handle, Some("显示全部"))?)
        .separator()
        .item(&PredefinedMenuItem::quit(handle, Some("退出 mydb"))?)
        .build()?;

    let new_connection = MenuItemBuilder::with_id("menu-new-connection", "新建连接")
        .accelerator("CmdOrCtrl+N")
        .build(handle)?;
    let new_query = MenuItemBuilder::with_id("menu-new-query", "新建查询")
        .accelerator("CmdOrCtrl+T")
        .build(handle)?;
    let open_sql = MenuItemBuilder::with_id("menu-open-sql-file", "打开 SQL 文件…")
        .accelerator("CmdOrCtrl+O")
        .build(handle)?;
    let save_sql = MenuItemBuilder::with_id("menu-save-sql", "保存 SQL")
        .accelerator("CmdOrCtrl+S")
        .build(handle)?;
    let save_sql_as = MenuItemBuilder::with_id("menu-save-sql-as", "另存为…")
        .accelerator("CmdOrCtrl+Shift+S")
        .build(handle)?;
    let reopen_tab = MenuItemBuilder::with_id("menu-reopen-closed-tab", "重新打开关闭的标签")
        .accelerator("CmdOrCtrl+Shift+T")
        .build(handle)?;
    let close_tab = MenuItemBuilder::with_id("menu-close-tab", "关闭标签")
        .accelerator("CmdOrCtrl+W")
        .build(handle)?;
    let close_window = MenuItemBuilder::with_id("menu-close-window", "关闭窗口")
        .accelerator("CmdOrCtrl+Shift+W")
        .build(handle)?;
    let import_data = MenuItemBuilder::with_id("menu-import-data", "导入数据…").build(handle)?;
    let export_result =
        MenuItemBuilder::with_id("menu-export-result", "导出结果…").build(handle)?;
    let export_database =
        MenuItemBuilder::with_id("menu-export-database", "导出数据库 SQL…").build(handle)?;
    let connection_transfer =
        MenuItemBuilder::with_id("menu-connection-transfer", "导入 / 导出连接…").build(handle)?;
    let backup_settings =
        MenuItemBuilder::with_id("menu-backup-settings", "备份计划…").build(handle)?;
    let execute_sql_file =
        MenuItemBuilder::with_id("menu-execute-sql-file", "执行 SQL 文件…").build(handle)?;

    let refresh = MenuItemBuilder::with_id("menu-refresh", "刷新对象")
        .accelerator("CmdOrCtrl+R")
        .build(handle)?;
    let history = MenuItemBuilder::with_id("menu-history", "执行历史")
        .accelerator("CmdOrCtrl+Y")
        .build(handle)?;
    let toggle_sidebar = CheckMenuItemBuilder::with_id("menu-toggle-sidebar", "连接侧栏")
        .accelerator("CmdOrCtrl+B")
        .checked(true)
        .build(handle)?;
    let toggle_objects = CheckMenuItemBuilder::with_id("menu-toggle-objects", "对象树")
        .accelerator("CmdOrCtrl+Shift+B")
        .checked(true)
        .build(handle)?;
    let command_palette =
        MenuItemBuilder::with_id("menu-command-palette", "命令面板…").build(handle)?;
    let toggle_result = CheckMenuItemBuilder::with_id("menu-toggle-result", "结果面板")
        .accelerator("CmdOrCtrl+J")
        .checked(true)
        .build(handle)?;
    let toggle_right = CheckMenuItemBuilder::with_id("menu-toggle-right", "右侧面板")
        .accelerator("CmdOrCtrl+Shift+H")
        .checked(true)
        .build(handle)?;
    let reset_layout = MenuItemBuilder::with_id("menu-reset-layout", "重置布局").build(handle)?;
    let fullscreen = PredefinedMenuItem::fullscreen(handle, Some("切换全屏"))?;

    let er_diagram = MenuItemBuilder::with_id("menu-er-diagram", "ER 图").build(handle)?;
    let structure_sync =
        MenuItemBuilder::with_id("menu-structure-sync", "结构同步…").build(handle)?;
    let data_sync = MenuItemBuilder::with_id("menu-data-sync", "数据同步…").build(handle)?;
    let data_transfer =
        MenuItemBuilder::with_id("menu-data-transfer", "数据传输…").build(handle)?;
    let query_builder =
        MenuItemBuilder::with_id("menu-query-builder", "查询构建器…").build(handle)?;
    let database_docs =
        MenuItemBuilder::with_id("menu-database-docs", "数据库文档…").build(handle)?;
    let server_activity =
        MenuItemBuilder::with_id("menu-server-activity", "服务器活动…").build(handle)?;
    let global_search =
        MenuItemBuilder::with_id("menu-global-search", "全局对象搜索…").build(handle)?;
    let edit_connection =
        MenuItemBuilder::with_id("menu-edit-connection", "编辑当前连接…").build(handle)?;
    let close_connection =
        MenuItemBuilder::with_id("menu-close-connection", "关闭当前连接").build(handle)?;
    let github = MenuItemBuilder::with_id("menu-github", "GitHub 项目主页").build(handle)?;
    let shortcut_help =
        MenuItemBuilder::with_id("menu-shortcut-help", "键盘快捷键").build(handle)?;
    let edit_search =
        MenuItemBuilder::with_id("menu-editor-search", "查找和替换…").build(handle)?;
    let edit_find_next =
        MenuItemBuilder::with_id("menu-editor-find-next", "查找下一个").build(handle)?;
    let edit_find_previous =
        MenuItemBuilder::with_id("menu-editor-find-previous", "查找上一个").build(handle)?;
    let edit_goto_line =
        MenuItemBuilder::with_id("menu-editor-goto-line", "转到行…").build(handle)?;
    let edit_toggle_comment =
        MenuItemBuilder::with_id("menu-editor-toggle-comment", "注释 / 取消注释").build(handle)?;
    let edit_check_sql =
        MenuItemBuilder::with_id("menu-check-sql", "检查 SQL").build(handle)?;
    let edit_format_sql =
        MenuItemBuilder::with_id("menu-editor-format-sql", "格式化 SQL").build(handle)?;
    let edit_snippets = MenuItemBuilder::with_id("menu-editor-snippets", "SQL 片段库…")
        .accelerator("CmdOrCtrl+Shift+K")
        .build(handle)?;
    let edit_save_snippet =
        MenuItemBuilder::with_id("menu-editor-save-snippet", "保存选区为片段…").build(handle)?;
    let edit_compress_sql =
        MenuItemBuilder::with_id("menu-editor-compress-sql", "压缩 SQL").build(handle)?;
    let edit_format_settings =
        MenuItemBuilder::with_id("menu-editor-format-settings", "格式化设置…").build(handle)?;

    let file_menu = SubmenuBuilder::new(handle, "文件")
        .item(&new_connection)
        .item(&new_query)
        .separator()
        .item(&open_sql)
        .item(&execute_sql_file)
        .item(&save_sql)
        .item(&save_sql_as)
        .item(&reopen_tab)
        .separator()
        .item(&import_data)
        .item(&export_result)
        .item(&export_database)
        .item(&connection_transfer)
        .item(&backup_settings)
        .separator()
        .item(&close_tab)
        .separator()
        .item(&close_window)
        .build()?;

    let edit_menu = SubmenuBuilder::new(handle, "编辑")
        .item(&PredefinedMenuItem::undo(handle, Some("撤销"))?)
        .item(&PredefinedMenuItem::redo(handle, Some("重做"))?)
        .separator()
        .item(&PredefinedMenuItem::cut(handle, Some("剪切"))?)
        .item(&PredefinedMenuItem::copy(handle, Some("复制"))?)
        .item(&PredefinedMenuItem::paste(handle, Some("粘贴"))?)
        .item(&PredefinedMenuItem::select_all(handle, Some("全选"))?)
        .separator()
        .item(&edit_search)
        .item(&edit_find_next)
        .item(&edit_find_previous)
        .item(&edit_goto_line)
        .item(&edit_toggle_comment)
        .item(&edit_check_sql)
        .item(&edit_format_sql)
        .item(&edit_compress_sql)
        .item(&edit_format_settings)
        .separator()
        .item(&edit_snippets)
        .item(&edit_save_snippet)
        .build()?;

    let view_menu = SubmenuBuilder::new(handle, "视图")
        .item(&refresh)
        .item(&history)
        .item(&command_palette)
        .separator()
        .item(&toggle_sidebar)
        .item(&toggle_objects)
        .item(&toggle_result)
        .item(&toggle_right)
        .separator()
        .item(&reset_layout)
        .separator()
        .item(&fullscreen)
        .build()?;

    let tools_menu = SubmenuBuilder::new(handle, "工具")
        .item(&er_diagram)
        .separator()
        .item(&structure_sync)
        .item(&data_sync)
        .item(&data_transfer)
        .item(&query_builder)
        .item(&database_docs)
        .item(&server_activity)
        .item(&global_search)
        .separator()
        .item(&edit_connection)
        .item(&close_connection)
        .build()?;

    let window_menu = SubmenuBuilder::new(handle, "窗口")
        .item(&PredefinedMenuItem::minimize(handle, Some("最小化"))?)
        .item(&PredefinedMenuItem::bring_all_to_front(
            handle,
            Some("前置全部窗口"),
        )?)
        .build()?;

    let help_menu = SubmenuBuilder::new(handle, "帮助")
        .item(&PredefinedMenuItem::about(handle, Some("关于 mydb"), None)?)
        .item(&shortcut_help)
        .separator()
        .item(&github)
        .build()?;

    MenuBuilder::new(handle)
        .items(&[
            &app_menu,
            &file_menu,
            &edit_menu,
            &view_menu,
            &tools_menu,
            &window_menu,
            &help_menu,
        ])
        .build()
}

#[derive(serde::Deserialize)]
struct MenuSyncState {
    enabled: std::collections::HashMap<String, bool>,
    checked: std::collections::HashMap<String, bool>,
}

fn sync_item_enabled<R: tauri::Runtime>(
    items: &[MenuItemKind<R>],
    id: &str,
    enabled: bool,
) -> bool {
    for item in items {
        if item.id().as_ref() == id {
            let _ = match item {
                MenuItemKind::MenuItem(item) => item.set_enabled(enabled),
                MenuItemKind::Check(item) => item.set_enabled(enabled),
                MenuItemKind::Icon(item) => item.set_enabled(enabled),
                _ => Ok(()),
            };
            return true;
        }
        if let MenuItemKind::Submenu(submenu) = item {
            if let Ok(children) = submenu.items() {
                if sync_item_enabled(&children, id, enabled) {
                    return true;
                }
            }
        }
    }
    false
}

fn sync_item_checked<R: tauri::Runtime>(
    items: &[MenuItemKind<R>],
    id: &str,
    checked: bool,
) -> bool {
    for item in items {
        if item.id().as_ref() == id {
            if let MenuItemKind::Check(item) = item {
                let _ = item.set_checked(checked);
            }
            return true;
        }
        if let MenuItemKind::Submenu(submenu) = item {
            if let Ok(children) = submenu.items() {
                if sync_item_checked(&children, id, checked) {
                    return true;
                }
            }
        }
    }
    false
}

/// Keep the native macOS menu's enabled/checked state in sync with the
/// in-window menu. Non-macOS builds have no app-wide menu, so this is a no-op.
#[tauri::command]
fn sync_menu_state(app: tauri::AppHandle, state: MenuSyncState) -> Result<(), String> {
    let Some(menu) = app.menu() else {
        return Ok(());
    };
    let items = menu.items().map_err(|error| error.to_string())?;
    for (id, enabled) in state.enabled {
        sync_item_enabled(&items, &id, enabled);
    }
    for (id, checked) in state.checked {
        sync_item_checked(&items, &id, checked);
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder.menu(build_menu).on_menu_event(|app, event| {
        let event_name = match event.id().as_ref() {
            "menu-new-connection" => "menu:new-connection",
            "menu-settings" => "menu:settings",
            "menu-new-query" => "menu:new-query",
            "menu-open-sql-file" => "menu:open-sql-file",
            "menu-save-sql" => "menu:save-sql",
            "menu-save-sql-as" => "menu:save-sql-as",
            "menu-reopen-closed-tab" => "menu:reopen-closed-tab",
            "menu-import-data" => "menu:import-data",
            "menu-export-result" => "menu:export-result",
            "menu-export-database" => "menu:export-database",
            "menu-connection-transfer" => "menu:connection-transfer",
            "menu-backup-settings" => "menu:backup-settings",
            "menu-execute-sql-file" => "menu:execute-sql-file",
            "menu-close-tab" => "menu:close-tab",
            "menu-close-window" => "menu:close-window",
            "menu-refresh" => "menu:refresh",
            "menu-history" => "menu:history",
            "menu-command-palette" => "menu:command-palette",
            "menu-toggle-sidebar" => "menu:toggle-sidebar",
            "menu-toggle-objects" => "menu:toggle-objects",
            "menu-toggle-result" => "menu:toggle-result",
            "menu-toggle-right" => "menu:toggle-right",
            "menu-reset-layout" => "menu:reset-layout",
            "menu-er-diagram" => "menu:er-diagram",
            "menu-structure-sync" => "menu:structure-sync",
            "menu-data-sync" => "menu:data-sync",
            "menu-data-transfer" => "menu:data-transfer",
            "menu-query-builder" => "menu:query-builder",
            "menu-database-docs" => "menu:database-docs",
            "menu-server-activity" => "menu:server-activity",
            "menu-global-search" => "menu:global-search",
            "menu-edit-connection" => "menu:edit-connection",
            "menu-close-connection" => "menu:close-connection",
            "menu-github" => "menu:github",
            "menu-shortcut-help" => "menu:shortcut-help",
            "menu-editor-search" => "menu:editor-search",
            "menu-editor-find-next" => "menu:editor-find-next",
            "menu-editor-find-previous" => "menu:editor-find-previous",
            "menu-editor-goto-line" => "menu:editor-goto-line",
            "menu-editor-toggle-comment" => "menu:editor-toggle-comment",
            "menu-check-sql" => "menu:check-sql",
            "menu-editor-format-sql" => "menu:editor-format-sql",
            "menu-editor-compress-sql" => "menu:editor-compress-sql",
            "menu-editor-format-settings" => "menu:editor-format-settings",
            "menu-editor-snippets" => "menu:editor-snippets",
            "menu-editor-save-snippet" => "menu:editor-save-snippet",
            _ => return,
        };
        // Menu actions belong to the window the user is looking at. Emitting to the
        // focused window keeps a standalone query window from moving the
        // main window's tabs (and vice versa).
        let focused = app
            .webview_windows()
            .values()
            .find(|window| window.is_focused().unwrap_or(false))
            .cloned();
        match focused {
            Some(window) => {
                let _ = window.emit(event_name, ());
            }
            None => {
                let _ = app.emit(event_name, ());
            }
        }
    });

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .setup(|app| {
            commands::start_scheduler(app.handle().clone());
            Ok(())
        })
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            sync_menu_state,
            commands::open_query_window,
            commands::connection_has_query_window,
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
            commands::clear_connection_secret,
            commands::close_connection,
            commands::duplicate_connection,
            commands::connection_info,
            commands::set_connection_read_only,
            commands::list_connection_groups,
            commands::save_connection_group,
            commands::rename_connection_group,
            commands::delete_connection_group,
            commands::move_connection_to_group,
            commands::test_connection,
            commands::check_connection_health,
            commands::reconnect_connection,
            commands::list_tables,
            commands::list_scope_columns,
            commands::list_databases,
            commands::create_database,
            commands::create_schema,
            commands::drop_database,
            commands::drop_schema,
            commands::list_database_objects,
            commands::list_schemas,
            commands::search_scope_objects,
            commands::get_connection_context,
            commands::list_context_tables,
            commands::list_context_schemas,
            commands::switch_database,
            commands::switch_schema,
            commands::execute_query,
            commands::cancel_query,
            commands::explain_query,
            commands::get_index_table,
            commands::begin_transaction,
            commands::execute_transaction_query,
            commands::commit_transaction,
            commands::rollback_transaction,
            commands::close_transaction_session,
            commands::list_columns,
            commands::get_table_structure,
            commands::get_er_diagram,
            commands::list_scope_foreign_keys,
            commands::database_docs_snapshot,
            commands::server_overview,
            commands::server_users,
            commands::server_user_grants,
            commands::server_processes,
            commands::terminate_server_process,
            commands::compare_schemas,
            commands::apply_schema_sync,
            commands::cancel_schema_sync,
            commands::compare_table_data,
            commands::apply_table_data_sync,
            commands::transfer_table_data,
            commands::cancel_data_transfer,
            commands::read_table_rows,
            commands::apply_table_changes,
            commands::preview_generated_data,
            commands::generate_table_data,
            commands::cancel_generate,
            commands::update_table_row,
            commands::insert_table_row,
            commands::delete_table_row,
            commands::alter_table_column,
            commands::get_object_ddl,
            commands::list_object_variants,
            commands::save_object_definition,
            commands::export_database_sql,
            commands::list_backup_jobs,
            commands::save_backup_job,
            commands::delete_backup_job,
            commands::run_backup_job,
            commands::cancel_backup,
            commands::list_backup_runs,
            commands::skip_missed_backups,
            commands::preview_sql_file,
            commands::execute_sql_file,
            commands::cancel_sql_script,
            commands::export_object,
            commands::export_table_rows,
            commands::export_query_result_xlsx,
            commands::cancel_export,
            commands::table_maintenance,
            commands::duplicate_object,
            commands::snapshot_view_to_table,
            commands::rename_object,
            commands::drop_object,
            commands::write_text_file,
            commands::read_text_file,
            commands::quit_app,
            commands::import_csv,
            commands::import_json,
            commands::preview_import_file,
            commands::execute_import_file,
            commands::cancel_import,
            commands::list_history,
            commands::clear_history,
            commands::redis_ping,
            commands::redis_databases,
            commands::redis_select_database,
            commands::redis_scan_keys,
            commands::redis_key_detail,
            commands::redis_delete_key,
            commands::redis_set_ttl,
            commands::redis_rename_key,
            commands::redis_set_string,
            commands::redis_hash_set,
            commands::redis_hash_delete_field,
            commands::redis_list_push,
            commands::redis_list_set,
            commands::redis_list_remove,
            commands::redis_set_add,
            commands::redis_set_remove,
            commands::redis_zset_set,
            commands::redis_zset_remove,
            commands::redis_command,
            commands::mongo_list_databases,
            commands::mongo_list_collections,
            commands::mongo_find_documents,
            commands::mongo_aggregate_documents,
            commands::mongo_insert_document,
            commands::mongo_insert_documents,
            commands::mongo_replace_document,
            commands::mongo_delete_document,
            commands::mongo_list_indexes,
            commands::mongo_create_index,
            commands::mongo_drop_index,
            commands::mongo_create_collection,
            commands::mongo_drop_collection,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
