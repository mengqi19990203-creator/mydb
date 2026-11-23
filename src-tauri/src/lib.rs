mod commands;
mod db;
mod history;
mod keychain;
mod models;
mod redis_client;
mod ssh;
mod storage;

use commands::AppState;
use tauri::menu::{
    MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::Emitter;

#[cfg(target_os = "macos")]
fn build_menu(handle: &tauri::AppHandle) -> tauri::Result<tauri::menu::Menu<tauri::Wry>> {
    let app_menu = SubmenuBuilder::new(handle, "mydb")
        .item(&PredefinedMenuItem::about(
            handle,
            Some("关于 mydb"),
            None,
        )?)
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
    let close_tab = MenuItemBuilder::with_id("menu-close-tab", "关闭标签")
        .accelerator("CmdOrCtrl+W")
        .build(handle)?;
    let close_window = MenuItemBuilder::with_id("menu-close-window", "关闭窗口")
        .accelerator("CmdOrCtrl+Shift+W")
        .build(handle)?;

    let refresh = MenuItemBuilder::with_id("menu-refresh", "刷新对象")
        .accelerator("CmdOrCtrl+R")
        .build(handle)?;
    let history = MenuItemBuilder::with_id("menu-history", "执行历史")
        .accelerator("CmdOrCtrl+Y")
        .build(handle)?;
    let toggle_sidebar = MenuItemBuilder::with_id("menu-toggle-sidebar", "连接侧栏")
        .accelerator("CmdOrCtrl+B")
        .build(handle)?;
    let toggle_objects = MenuItemBuilder::with_id("menu-toggle-objects", "对象树")
        .accelerator("CmdOrCtrl+Shift+B")
        .build(handle)?;
    let toggle_result = MenuItemBuilder::with_id("menu-toggle-result", "结果面板")
        .accelerator("CmdOrCtrl+J")
        .build(handle)?;
    let toggle_right = MenuItemBuilder::with_id("menu-toggle-right", "右侧面板")
        .accelerator("CmdOrCtrl+Shift+H")
        .build(handle)?;
    let reset_layout = MenuItemBuilder::with_id("menu-reset-layout", "重置布局")
        .build(handle)?;

    let er_diagram = MenuItemBuilder::with_id("menu-er-diagram", "ER 图").build(handle)?;
    let structure_sync =
        MenuItemBuilder::with_id("menu-structure-sync", "结构同步…").build(handle)?;
    let data_sync = MenuItemBuilder::with_id("menu-data-sync", "数据同步…").build(handle)?;

    let file_menu = SubmenuBuilder::new(handle, "文件")
        .item(&new_connection)
        .item(&new_query)
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
        .build()?;

    let view_menu = SubmenuBuilder::new(handle, "视图")
        .item(&refresh)
        .item(&history)
        .separator()
        .item(&toggle_sidebar)
        .item(&toggle_objects)
        .item(&toggle_result)
        .item(&toggle_right)
        .separator()
        .item(&reset_layout)
        .build()?;

    let tools_menu = SubmenuBuilder::new(handle, "工具")
        .item(&er_diagram)
        .separator()
        .item(&structure_sync)
        .item(&data_sync)
        .build()?;

    let window_menu = SubmenuBuilder::new(handle, "窗口")
        .item(&PredefinedMenuItem::minimize(handle, Some("最小化"))?)
        .item(&PredefinedMenuItem::fullscreen(handle, Some("全屏"))?)
        .item(&PredefinedMenuItem::bring_all_to_front(
            handle,
            Some("前置全部窗口"),
        )?)
        .build()?;

    let help_menu = SubmenuBuilder::new(handle, "帮助")
        .item(&PredefinedMenuItem::about(
            handle,
            Some("关于 mydb"),
            None,
        )?)
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

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();

    #[cfg(target_os = "macos")]
    let builder = builder
        .menu(build_menu)
        .on_menu_event(|app, event| {
            let event_name = match event.id().as_ref() {
                "menu-new-connection" => "menu:new-connection",
                "menu-new-query" => "menu:new-query",
                "menu-close-tab" => "menu:close-tab",
                "menu-close-window" => "menu:close-window",
                "menu-refresh" => "menu:refresh",
                "menu-history" => "menu:history",
                "menu-toggle-sidebar" => "menu:toggle-sidebar",
                "menu-toggle-objects" => "menu:toggle-objects",
                "menu-toggle-result" => "menu:toggle-result",
                "menu-toggle-right" => "menu:toggle-right",
                "menu-reset-layout" => "menu:reset-layout",
                "menu-er-diagram" => "menu:er-diagram",
                "menu-structure-sync" => "menu:structure-sync",
                "menu-data-sync" => "menu:data-sync",
                _ => return,
            };
            let _ = app.emit(event_name, ());
        });

    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::list_connections,
            commands::save_connection,
            commands::delete_connection,
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
            commands::list_tables,
            commands::list_scope_columns,
            commands::list_databases,
            commands::list_database_objects,
            commands::list_schemas,
            commands::get_connection_context,
            commands::list_context_tables,
            commands::list_context_schemas,
            commands::switch_database,
            commands::switch_schema,
            commands::execute_query,
            commands::get_index_table,
            commands::begin_transaction,
            commands::execute_transaction_query,
            commands::commit_transaction,
            commands::rollback_transaction,
            commands::close_transaction_session,
            commands::list_columns,
            commands::get_table_structure,
            commands::get_er_diagram,
            commands::compare_schemas,
            commands::compare_table_data,
            commands::apply_table_data_sync,
            commands::read_table_rows,
            commands::update_table_row,
            commands::insert_table_row,
            commands::delete_table_row,
            commands::get_object_ddl,
            commands::export_object,
            commands::cancel_export,
            commands::table_maintenance,
            commands::duplicate_object,
            commands::snapshot_view_to_table,
            commands::rename_object,
            commands::drop_object,
            commands::write_text_file,
            commands::import_csv,
            commands::import_json,
            commands::list_history,
            commands::clear_history,
            commands::redis_ping,
            commands::redis_databases,
            commands::redis_select_database,
            commands::redis_scan_keys,
            commands::redis_key_detail,
            commands::redis_delete_key,
            commands::redis_command,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
