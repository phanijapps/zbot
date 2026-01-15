// ============================================================================
// AGENT ZERO - Tauri Backend
// Main entry point with modular command handlers
// ============================================================================

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Modules
mod commands;
mod settings;
mod domains;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            // Initialize application directories on startup
            use settings::AppDirs;

            match AppDirs::get() {
                Ok(dirs) => {
                    if let Err(e) = dirs.initialize() {
                        eprintln!("Failed to initialize directories: {}", e);
                    }

                    // Create default settings file if it doesn't exist
                    if !dirs.settings_file.exists() {
                        if let Err(e) = dirs.save_settings(&settings::Settings::default()) {
                            eprintln!("Failed to create default settings: {}", e);
                        }
                    }

                    // Log the config directory for debugging
                    println!("Agent Zero config directory: {:?}", dirs.config_dir);
                }
                Err(e) => {
                    eprintln!("Failed to get app directories: {}", e);
                }
            }

            // Initialize conversation database
            if let Err(e) = domains::conversation_runtime::init_database() {
                eprintln!("Failed to initialize conversation database: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Core commands
            commands::greet,
            commands::get_app_info,
            // Conversation commands
            commands::list_conversations,
            commands::get_conversation,
            commands::create_conversation,
            commands::update_conversation,
            commands::delete_conversation,
            commands::list_messages,
            commands::create_message,
            commands::get_message,
            commands::delete_message,
            commands::get_conversation_stats,
            // Agent commands
            commands::list_agents,
            commands::get_agent,
            commands::create_agent,
            commands::update_agent,
            commands::delete_agent,
            commands::list_agent_files,
            commands::read_agent_file,
            commands::write_agent_file,
            commands::create_agent_folder,
            commands::delete_agent_file,
            commands::upload_agent_file,
            // Provider commands
            commands::list_providers,
            commands::get_provider,
            commands::create_provider,
            commands::update_provider,
            commands::delete_provider,
            commands::test_provider,
            // MCP commands
            commands::list_mcp_servers,
            commands::get_mcp_server,
            commands::create_mcp_server,
            commands::update_mcp_server,
            commands::delete_mcp_server,
            commands::start_mcp_server,
            commands::stop_mcp_server,
            commands::test_mcp_server,
            // Skill commands
            commands::list_skills,
            commands::get_skill,
            commands::create_skill,
            commands::update_skill,
            commands::delete_skill,
            commands::list_skill_files,
            commands::read_skill_file,
            commands::write_skill_file,
            commands::create_skill_folder,
            commands::delete_skill_file,
            // Settings commands
            commands::get_settings,
            commands::save_settings,
            commands::reset_settings,
            commands::get_storage_info,
            commands::clear_all_data,
            commands::get_config_path,
            commands::initialize_directories,
            // Window commands
            commands::open_skill_editor_window,
            commands::open_external,
            // Tool commands
            commands::read_file_lines,
            commands::write_file_with_dirs,
            commands::execute_shell_command,
            commands::execute_python_code,
            commands::grep_files,
            commands::glob_files,
            // Agent Runtime commands
            commands::execute_agent_stream,
            commands::get_agent_execution_config,
            commands::create_agent_conversation,
            commands::get_or_create_conversation,
            commands::clear_executor_cache,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
