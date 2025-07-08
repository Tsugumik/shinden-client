// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/

use shinden_pl_api::client;
use shinden_pl_api::client::ShindenAPI;
use shinden_pl_api::models::Anime;

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

#[tauri::command]
async fn search(state: tauri::State<'_, Api>, query: String) -> Result<Vec<Anime>, String> {
    state.0.search_anime(&query).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn login(state: tauri::State<'_, Api>, username: String, password: String) -> Result<(), String> {
    state.0.login(&username, &password).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user_name(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state.0.get_user_name().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user_profile_image(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state.0.get_user_profile_image().await.map_err(|e| e.to_string())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .manage(Api(ShindenAPI::new().expect("Failed to create ShindenAPI")))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![greet, test_connection, search, login, get_user_name, get_user_profile_image])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
