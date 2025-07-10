
use reqwest::Client;
// Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
use shinden_pl_api::client::ShindenAPI;
use shinden_pl_api::models::{Anime, Episode, Player};
use warp::{Filter, http::header::{HeaderMap, HeaderValue, RANGE, CONTENT_TYPE, ACCESS_CONTROL_ALLOW_ORIGIN}};
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
    state
        .0
        .search_anime(&query)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn login(
    state: tauri::State<'_, Api>,
    username: String,
    password: String,
) -> Result<(), String> {
    state
        .0
        .login(&username, &password)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn logout(state: tauri::State<'_, Api>) -> Result<(), String> {
    state.0.logout().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user_name(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state.0.get_user_name().await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_user_profile_image(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state
        .0
        .get_user_profile_image()
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_episodes(state: tauri::State<'_, Api>, url: String) -> Result<Vec<Episode>, String> {
    state.0.get_episodes(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_players(state: tauri::State<'_, Api>, url: String) -> Result<Vec<Player>, String> {
    state.0.get_players(&url).await.map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_iframe(state: tauri::State<'_, Api>, id: String) -> Result<String, String> {
    state
        .0
        .get_player_iframe(&id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
async fn get_cda_video(state: tauri::State<'_, Api>, url: String) -> Result<String, String> {
    let url_clone = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            cda_dl::get_video_url(&url_clone)
                .await
                .map_err(|e| e.to_string())
        })
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_http::init())
        .manage(Api(ShindenAPI::new().expect("Failed to create ShindenAPI")))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            test_connection,
            search,
            login,
            get_user_name,
            get_user_profile_image,
            logout,
            get_episodes,
            get_players,
            get_iframe,
            get_cda_video
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
