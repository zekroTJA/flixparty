mod client;
mod model;
mod retry;

struct AppState {}

#[tauri::command]
fn connect(address: &str, channel: &str) -> Result<(), model::Error> {
    let client = Client::new(address, channel);
    let conn = client.connect()?;

    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_writer(std::io::stdout)
        .init();

    let h = PeripheryHandler::new();
    h.listen().unwrap();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![connect])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
