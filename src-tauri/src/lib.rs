// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use shinden_pl_api::client;
use shinden_pl_api::client::ShindenAPI;

struct Api(ShindenAPI);

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
async fn test_connection(state: tauri::State<'_, Api>) -> Result<(), String> {
    match state.0.get_html("http://shinden.pl").await {
        Ok(_) => Ok(()),
        Err(e) => Err(format!("Connection failed: {}", e)),
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Api(ShindenAPI::new().expect("Failed to create ShindenAPI")))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, test_connection])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
