use shinden_pl_api::client::ShindenAPI;
use shinden_pl_api::models::{Anime, Episode, Player};
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

struct Api(ShindenAPI);

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn write_log(level: String, message: String) -> Result<(), String> {
    discard_log_path(append_project_log(&level, &message))
}

#[tauri::command]
async fn test_connection(state: tauri::State<'_, Api>) -> Result<(), String> {
    match state.0.get_html("http://shinden.pl").await {
        Ok(_) => Ok(()),
        Err(e) => Err(command_error(
            "test_connection",
            format!("Connection failed: {}", e),
        )),
    }
}

#[tauri::command]
async fn search(state: tauri::State<'_, Api>, query: String) -> Result<Vec<Anime>, String> {
    state
        .0
        .search_anime(&query)
        .await
        .map_err(|e| command_error("search", e))
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
        .map_err(|e| command_error("login", e))
}

#[tauri::command]
async fn logout(state: tauri::State<'_, Api>) -> Result<(), String> {
    state.0.logout().await.map_err(|e| command_error("logout", e))
}

#[tauri::command]
async fn get_user_name(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state
        .0
        .get_user_name()
        .await
        .map_err(|e| command_error("get_user_name", e))
}

#[tauri::command]
async fn get_user_profile_image(state: tauri::State<'_, Api>) -> Result<Option<String>, String> {
    state
        .0
        .get_user_profile_image()
        .await
        .map_err(|e| command_error("get_user_profile_image", e))
}

#[tauri::command]
async fn get_episodes(state: tauri::State<'_, Api>, url: String) -> Result<Vec<Episode>, String> {
    state
        .0
        .get_episodes(&url)
        .await
        .map_err(|e| command_error("get_episodes", e))
}

#[tauri::command]
async fn get_players(state: tauri::State<'_, Api>, url: String) -> Result<Vec<Player>, String> {
    state
        .0
        .get_players(&url)
        .await
        .map_err(|e| command_error("get_players", e))
}

#[tauri::command]
async fn get_iframe(state: tauri::State<'_, Api>, id: String) -> Result<String, String> {
    state
        .0
        .get_player_iframe(&id)
        .await
        .map_err(|e| command_error("get_iframe", e))
}

#[tauri::command]
async fn get_cda_video(_state: tauri::State<'_, Api>, url: String) -> Result<String, String> {
    let url_clone = url.clone();
    tauri::async_runtime::spawn_blocking(move || {
        tauri::async_runtime::block_on(async {
            cda_dl::get_video_url(&url_clone)
                .await
                .map_err(|e| command_error("get_cda_video", e))
        })
    })
    .await
    .map_err(|e| command_error("get_cda_video task", e))?
}

fn command_error<E: ToString>(context: &str, error: E) -> String {
    let message = error.to_string();
    let _ = append_project_log("ERROR", &format!("{context}: {message}"));
    message
}

fn append_project_log(level: &str, message: &str) -> io::Result<PathBuf> {
    append_log_line(&resolve_project_log_dir(), level, message)
}

fn discard_log_path(result: io::Result<PathBuf>) -> Result<(), String> {
    result.map(|_| ()).map_err(|e| e.to_string())
}

fn append_log_line(log_dir: &Path, level: &str, message: &str) -> io::Result<PathBuf> {
    fs::create_dir_all(log_dir)?;
    let log_file = log_dir.join("shinden-client.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_file)?;
    writeln!(file, "{} [{level}] {message}", unix_timestamp_ms())?;
    Ok(log_file)
}

fn unix_timestamp_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis())
        .unwrap_or_default()
}

fn resolve_project_log_dir() -> PathBuf {
    if let Ok(path) = std::env::var("SHINDEN_CLIENT_LOG_DIR") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    if let Some(root) = option_env!("SHINDEN_BUILD_PROJECT_ROOT") {
        let path = PathBuf::from(root);
        if is_project_root(&path) {
            return path.join("logs");
        }
    }

    let mut starts = Vec::new();
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            starts.push(parent.to_path_buf());
        }
    }
    if let Ok(current_dir) = std::env::current_dir() {
        starts.push(current_dir);
    }

    for start in starts {
        if let Some(root) = find_project_root_from(&start) {
            return root.join("logs");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("logs")
}

fn find_project_root_from(start: &Path) -> Option<PathBuf> {
    start.ancestors().find(|path| is_project_root(path)).map(PathBuf::from)
}

fn is_project_root(path: &Path) -> bool {
    path.join("package.json").is_file() && path.join("src-tauri").join("tauri.conf.json").is_file()
}

fn install_panic_logger() {
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload = panic_info
            .payload()
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic_info.payload().downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown panic payload");
        let location = panic_info
            .location()
            .map(|location| format!("{}:{}", location.file(), location.line()))
            .unwrap_or_else(|| "unknown location".to_string());
        let _ = append_project_log("PANIC", &format!("{payload} at {location}"));
        previous_hook(panic_info);
    }));
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    install_panic_logger();

    let api = match ShindenAPI::new() {
        Ok(api) => api,
        Err(error) => {
            let _ = append_project_log("FATAL", &format!("Failed to create ShindenAPI: {error}"));
            panic!("Failed to create ShindenAPI: {error}");
        }
    };

    if let Err(error) = tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_http::init())
        .manage(Api(api))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            write_log,
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
    {
        let _ = append_project_log(
            "FATAL",
            &format!("error while running tauri application: {error}"),
        );
        panic!("error while running tauri application: {error}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn unique_temp_dir(name: &str) -> std::path::PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock should be after UNIX_EPOCH")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "shinden_client_{}_{}_{}",
            name,
            std::process::id(),
            stamp
        ))
    }

    #[test]
    fn find_project_root_from_detects_repository_markers() {
        let root = unique_temp_dir("root_markers");
        let nested = root.join("src-tauri").join("target").join("release");
        fs::create_dir_all(&nested).expect("failed to create nested test directory");
        fs::write(root.join("package.json"), "{}").expect("failed to create package marker");
        fs::write(root.join("src-tauri").join("tauri.conf.json"), "{}")
            .expect("failed to create tauri marker");

        let found = find_project_root_from(&nested);

        assert_eq!(found.as_deref(), Some(root.as_path()));
        fs::remove_dir_all(root).expect("failed to remove test directory");
    }

    #[test]
    fn append_log_line_writes_exceptions_to_project_log_file() {
        let log_dir = unique_temp_dir("logs");

        let path = append_log_line(&log_dir, "ERROR", "example exception")
            .expect("failed to append log line");

        assert_eq!(path, log_dir.join("shinden-client.log"));
        let contents = fs::read_to_string(path).expect("failed to read log file");
        assert!(contents.contains("[ERROR] example exception"));
        fs::remove_dir_all(log_dir).expect("failed to remove log directory");
    }

    #[test]
    fn write_log_command_discards_log_file_path() {
        let result: Result<(), String> =
            discard_log_path(Ok(PathBuf::from("shinden-client.log")));

        assert_eq!(result, Ok(()));
    }
}
