/// Returns the kernel HTTP port (used by frontend to construct API URLs).
#[tauri::command]
fn get_kernel_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(8081)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Tauri desktop mode: bind kernel to loopback only for security
    std::env::set_var("BIND_ADDRESS", "127.0.0.1");

    // Add Tauri WebView origins to CORS allowlist
    let existing_cors = std::env::var("CORS_ORIGINS").unwrap_or_default();
    let tauri_origins = "tauri://localhost,http://tauri.localhost";
    let combined = if existing_cors.is_empty() {
        format!("http://localhost:5173,http://127.0.0.1:5173,{}", tauri_origins)
    } else {
        format!("{},{}", existing_cors, tauri_origins)
    };
    std::env::set_var("CORS_ORIGINS", combined);

    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![get_kernel_port])
        .setup(|app| {
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            // Launch the Exiv Kernel Server in a background async task
            tauri::async_runtime::spawn(async move {
                dotenvy::dotenv().ok();
                if let Err(e) = exiv_core::run_kernel().await {
                    eprintln!("Failed to start Exiv Kernel: {}", e);
                }
            });

            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
