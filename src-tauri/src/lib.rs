// ============================================================================
// AGENT ZERO - Tauri Backend
// Main entry point with modular command handlers
// ============================================================================

// Prevents additional console window on Windows in release
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Modules
mod audio_recorder;
mod commands;
mod settings;
mod domains;
mod transcription;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Initialize structured logging with controlled output
    // Logs are written to file and stderr based on RUST_LOG environment variable
    agent_runtime::init_logging(agent_runtime::LogLevel::Info, true);

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|_app| {
            // Initialize application directories on startup
            use settings::AppDirs;

            match AppDirs::get() {
                Ok(dirs) => {
                    if let Err(e) = dirs.initialize() {
                        tracing::error!("Failed to initialize directories: {}", e);
                    }

                    // Create default settings file if it doesn't exist
                    if !dirs.settings_file.exists() {
                        if let Err(e) = dirs.save_settings(&settings::Settings::default()) {
                            tracing::error!("Failed to create default settings: {}", e);
                        }
                    }

                    // Log the config directory for debugging
                    tracing::info!("Agent Zero config directory: {:?}", dirs.config_dir);
                }
                Err(e) => {
                    tracing::error!("Failed to get app directories: {}", e);
                }
            }

            // Initialize conversation database
            if let Err(e) = domains::conversation_runtime::init_database() {
                tracing::error!("Failed to initialize conversation database: {}", e);
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            // Core commands
            commands::greet,
            commands::get_app_info,
            // Vault commands
            commands::list_vaults,
            commands::get_active_vault,
            commands::create_vault,
            commands::switch_vault,
            commands::delete_vault,
            commands::get_vault_info,
            commands::initialize_vault_system,
            commands::get_vault_status,
            commands::set_default_vault,
            // Deletion & Cache commands (NEW)
            commands::delete_session,
            commands::delete_agent_history_with_scope,
            commands::get_cache_stats,
            commands::clear_cache,
            commands::invalidate_session_cache,
            commands::invalidate_agent_cache,
            // Search commands (NEW)
            commands::initialize_search_index,
            commands::search_messages,
            commands::index_message,
            commands::index_messages,
            commands::rebuild_search_index,
            commands::delete_session_from_index,
            commands::delete_agent_from_index,
            commands::clear_search_index,
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
            commands::get_agent_flow_config,
            commands::save_agent_flow_config,
            // Subagent commands
            commands::list_subagents,
            commands::get_subagent,
            commands::save_subagent,
            commands::delete_subagent,
            // Workflow commands (NEW - XY Flow integration)
            commands::get_orchestrator_structure,
            commands::save_orchestrator_structure,
            commands::validate_workflow,
            commands::execute_workflow,
            commands::stop_workflow,
            // Agent Channel commands (NEW)
            commands::get_or_create_today_session,
            commands::list_previous_days,
            commands::load_session_messages,
            commands::delete_agent_history,
            commands::record_session_message,
            commands::generate_session_summary,
            commands::list_agent_channels,
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
            commands::get_skill_metadata,
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
            // Python venv commands
            commands::get_venv_info,
            commands::read_requirements,
            commands::save_requirements,
            commands::install_requirements,
            commands::list_installed_packages,
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
            commands::write_attachment_file,
            commands::read_attachment_file,
            // Agent Runtime commands
            commands::execute_agent_stream,
            commands::get_agent_execution_config,
            commands::create_agent_conversation,
            commands::get_or_create_conversation,
            commands::clear_executor_cache,
            // Knowledge Graph commands
            commands::get_knowledge_graph,
            commands::get_knowledge_graph_entities,
            commands::get_knowledge_graph_relationships,
            // Media commands
            commands::get_audio_input_devices,
            commands::start_audio_recording,
            commands::stop_audio_recording,
            commands::is_recording_audio,
            commands::save_audio_recording,
            commands::add_recording_to_kg,
            // Transcription commands
            commands::install_transcription_script,
            commands::check_transcription_dependencies,
            commands::transcribe_recording,
            commands::get_recording_transcript,
            commands::has_transcript,
            commands::get_transcript_attachment_info,
            // Attachment commands
            commands::list_attachments,
            commands::get_attachment,
            commands::delete_attachment,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
