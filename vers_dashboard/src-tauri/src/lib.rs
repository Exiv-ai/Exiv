#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_shell::init()) // Shell plugin for sidecars
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      // 🚀 Launch the VERS Kernel Server in a separate async task
      tauri::async_runtime::spawn(async move {
        dotenvy::dotenv().ok();
        if let Err(e) = vers_core::run_kernel().await {
            eprintln!("❌ Failed to start VERS Kernel: {}", e);
        }
      });

      Ok(())
    })
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
