use futures_util::stream::{self, StreamExt};
use reqwest::header::{ACCEPT, CONTENT_TYPE, ORIGIN, REFERER};
use serde::{Deserialize, Serialize};
use shinden_pl_api::client::ShindenAPI;
use shinden_pl_api::models::{Anime, Episode, Player};
use std::collections::HashMap;
use std::fs::{self, OpenOptions};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const WATCHING_LIST_PAGE_LIMIT: usize = 100;
const WATCHING_LIST_STATUSES: [&str; 6] = [
    "in progress",
    "completed",
    "skip",
    "hold",
    "dropped",
    "plan",
];
const WATCHING_CACHE_TTL_MS: u64 = 30 * 60 * 1000;
const WATCHING_CACHE_REFRESH_CONCURRENCY: usize = 4;
const WATCHING_CACHE_REQUEST_RETRIES: usize = 2;
const WATCHING_CACHE_RETRY_DELAY_MS: u64 = 750;
const USER_ID_CACHE_TTL_MS: u64 = 60 * 60 * 1000;
const SHINDEN_TITLE_PLACEHOLDER: &str =
    "https://shinden.pl/res/other/placeholders/title/100x100.jpg";

struct Api(
    ShindenAPI,
    Mutex<WatchingCacheRefreshStatus>,
    Mutex<CachedUserId>,
);

#[derive(Debug, Deserialize)]
struct WatchingListApiResponse {
    success: bool,
    result: WatchingListApiResult,
}

#[derive(Debug, Deserialize)]
struct WatchingListApiResult {
    count: usize,
    items: Vec<WatchingListApiItem>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleEpisodesApiResponse {
    success: bool,
    message: Option<String>,
    result: TitleEpisodesApiResult,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleEpisodesApiResult {
    count: u32,
    items: Vec<TitleEpisodeApiItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TitleEpisodeApiItem {
    episode_id: u64,
    episode_no: u32,
    is_filer: Option<u8>,
    watched: Option<TitleEpisodeWatchedApiItem>,
    #[serde(rename = "titlePL")]
    title_pl: Option<TitleEpisodeTitleApiItem>,
    #[serde(rename = "titleEN")]
    title_en: Option<TitleEpisodeTitleApiItem>,
    #[serde(rename = "titleOfficial")]
    title_official: Option<TitleEpisodeTitleApiItem>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TitleEpisodeWatchedApiItem {
    episode_id: u64,
    view_cnt: u32,
    created_time: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TitleEpisodeTitleApiItem {
    lang: String,
    episode_id: u64,
    title: String,
    title_type: String,
}

#[derive(Debug, Deserialize)]
struct ShindenWriteResponse {
    success: bool,
    message: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TitleStatusApiResponse {
    success: bool,
    message: Option<String>,
    result: TitleStatusApiResult,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct TitleStatusApiResult {
    title: Option<TitleStatusApiTitle>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct TitleStatusApiTitle {
    watch_status: Option<String>,
    is_favourite: Option<u8>,
    priority: Option<i32>,
    recommend: Option<i32>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct WatchingListApiItem {
    title_id: u64,
    watch_status: Option<String>,
    is_favourite: Option<u8>,
    title: String,
    cover_id: Option<u64>,
    anime_type: Option<String>,
    summary_rating_total: Option<String>,
    episodes: Option<u32>,
    watched_episodes_cnt: Option<String>,
    description_pl: Option<String>,
    description_en: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WatchingAnimeFilter {
    only_available_unwatched: Option<bool>,
    subtitle_language: Option<String>,
    check_subtitle_availability_online: Option<bool>,
    exclude_ai_subtitles: Option<bool>,
}

impl WatchingAnimeFilter {
    fn only_available_unwatched(&self) -> bool {
        self.only_available_unwatched.unwrap_or(false)
    }

    fn subtitle_language(&self) -> &str {
        self.subtitle_language.as_deref().unwrap_or_default()
    }

    fn check_subtitle_availability_online(&self) -> bool {
        self.check_subtitle_availability_online.unwrap_or(false)
    }

    fn exclude_ai_subtitles(&self) -> bool {
        self.exclude_ai_subtitles.unwrap_or(false)
    }
}

#[derive(Debug, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WatchingAvailabilityCache {
    entries: HashMap<String, WatchingAvailabilityCacheEntry>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct WatchingAvailabilityCacheEntry {
    title_id: u64,
    watched_episodes_cnt: u32,
    total_episodes: Option<u32>,
    has_available_unwatched_episode: bool,
    subtitle_availability: HashMap<String, bool>,
    checked_at_ms: u64,
}

#[derive(Debug, Serialize, Clone, Default)]
#[serde(rename_all = "camelCase")]
struct WatchingCacheRefreshStatus {
    running: bool,
    current: usize,
    total: usize,
    refreshed: usize,
    skipped: usize,
    failed: usize,
    current_title: String,
    last_finished_at_ms: Option<u64>,
    last_error: Option<String>,
}

#[derive(Debug, Serialize, Default)]
#[serde(rename_all = "camelCase")]
struct WatchingCacheRefreshSummary {
    status: WatchingCacheRefreshStatus,
    already_running: bool,
}

struct WatchingCacheRefreshPlan {
    items_to_scan: Vec<WatchingListApiItem>,
    skipped: usize,
    processed: usize,
}

#[derive(Debug, Clone, Default)]
struct CachedUserId {
    user_id: Option<String>,
    checked_at_ms: u64,
}

#[derive(Debug, Serialize, Clone)]
struct WatchingAnime {
    #[serde(rename = "titleId")]
    title_id: u64,
    name: String,
    url: String,
    image_url: String,
    anime_type: String,
    rating: String,
    episodes: String,
    description: String,
    #[serde(rename = "watchStatus")]
    watch_status: String,
    #[serde(rename = "isFavourite")]
    is_favourite: u8,
    #[serde(rename = "watchedEpisodesCount")]
    watched_episodes_count: u32,
    #[serde(rename = "totalEpisodes")]
    total_episodes: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SearchAnime {
    #[serde(flatten)]
    anime: Anime,
    title_id: Option<u64>,
    watch_status: String,
    is_favourite: u8,
    total_episodes: Option<u32>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
struct EpisodeProgress {
    title: String,
    link: String,
    episode_id: Option<u64>,
    episode_no: u32,
    watched: bool,
    view_count: u32,
    total_episodes: Option<u32>,
    is_true_final_episode: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TitleStatusChangePayload {
    input: Vec<TitleStatusChangeInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TitleStatusChangeInput {
    title_id: u64,
    watch_status: Option<&'static str>,
    is_favourite: u8,
    priority: i32,
    recommend: i32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WatchedEpisodesChangePayload {
    title_id: u64,
    episodes: Vec<WatchedEpisodeChangeInput>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WatchedEpisodeChangeInput {
    episode_id: u64,
    view_cnt: u32,
    created_time: String,
}

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
async fn search(state: tauri::State<'_, Api>, query: String) -> Result<Vec<SearchAnime>, String> {
    let results = state
        .0
        .search_anime(&query)
        .await
        .map_err(|e| command_error("search", e))?;

    let watching_items = fetch_all_userlist_items(&state.0, &state.2)
        .await
        .unwrap_or_default();

    Ok(map_search_anime_results(results, watching_items))
}

#[tauri::command]
async fn get_watching_anime(
    state: tauri::State<'_, Api>,
    filter: Option<WatchingAnimeFilter>,
) -> Result<Vec<WatchingAnime>, String> {
    let filter = filter.unwrap_or_default();
    let cache = load_watching_availability_cache();
    let items = fetch_all_watching_items(&state.0, &state.2).await?;

    Ok(items
        .into_iter()
        .filter(|item| watching_cache_filter_matches(item, &filter, &cache))
        .filter_map(map_watching_list_item_details)
        .collect())
}

#[tauri::command]
async fn get_episodes_with_progress(
    state: tauri::State<'_, Api>,
    url: String,
    title_id: Option<u64>,
    total_episodes: Option<u32>,
) -> Result<Vec<EpisodeProgress>, String> {
    let playback_episodes = state
        .0
        .get_episodes(&url)
        .await
        .map_err(|e| command_error("get_episodes_with_progress playback", e))?;

    let Some(title_id) = title_id.or_else(|| {
        title_id_from_series_url(&url).and_then(|title_id| title_id.parse::<u64>().ok())
    }) else {
        return Ok(merge_episode_progress(playback_episodes, Vec::new(), total_episodes));
    };

    let progress_episodes = match fetch_current_user_id_cached(
        &state.0,
        &state.2,
        "get_episodes_with_progress",
    )
    .await
    {
        Ok(user_id) => fetch_title_episode_progress(&state.0, title_id, &user_id)
            .await
            .unwrap_or_else(|error| {
                let _ = command_error("get_episodes_with_progress progress fallback", error);
                Vec::new()
            }),
        Err(error) => {
            let _ = command_error("get_episodes_with_progress user fallback", error);
            Vec::new()
        }
    };

    Ok(merge_episode_progress(
        playback_episodes,
        progress_episodes,
        total_episodes,
    ))
}

#[tauri::command]
async fn update_anime_status(
    state: tauri::State<'_, Api>,
    title_id: u64,
    status: Option<String>,
    is_favourite: Option<u8>,
) -> Result<(), String> {
    let user_id = fetch_current_user_id_cached(&state.0, &state.2, "update_anime_status").await?;
    let current_status = fetch_title_status(&state.0, title_id, &user_id)
        .await
        .unwrap_or_default();
    let payload = build_title_status_payload_with_details(
        title_id,
        status.as_deref(),
        is_favourite.or_else(|| {
            current_status
                .as_ref()
                .and_then(|status| status.is_favourite)
        }),
        current_status
            .as_ref()
            .and_then(|status| status.priority)
            .unwrap_or_default(),
        current_status
            .as_ref()
            .and_then(|status| status.recommend)
            .unwrap_or_default(),
    )?;

    post_shinden_json(
        &state.0,
        "https://lista.shinden.pl/api/title-status-change",
        &payload,
        "update_anime_status",
    )
    .await?;

    match verify_title_status_change_with_user(
        &state.0,
        title_id,
        &user_id,
        status.as_deref(),
        "update_anime_status",
    )
    .await
    {
        Ok(()) => Ok(()),
        Err(verify_error) => {
            let _ = append_project_log(
                "WARNING",
                &format!("update_anime_status fallback after failed list verification: {verify_error}"),
            );
            post_legacy_anime_status(
                &state.0,
                title_id,
                &user_id,
                status.as_deref(),
                &payload.input[0],
            )
            .await?;
            verify_title_status_change_with_user(
                &state.0,
                title_id,
                &user_id,
                status.as_deref(),
                "update_anime_status legacy verify",
            )
            .await
        }
    }
}

#[tauri::command]
async fn mark_episode_watched(
    state: tauri::State<'_, Api>,
    title_id: u64,
    episode_id: u64,
    created_time: String,
) -> Result<(), String> {
    let payload = build_watched_episode_payload(title_id, episode_id, created_time, 1);
    post_shinden_json(
        &state.0,
        "https://lista.shinden.pl/api/title-watched-episodes-change",
        &payload,
        "mark_episode_watched",
    )
    .await
}

#[tauri::command]
async fn mark_episode_unwatched(
    state: tauri::State<'_, Api>,
    title_id: u64,
    episode_id: u64,
    created_time: String,
) -> Result<(), String> {
    let payload = build_watched_episode_payload(title_id, episode_id, created_time, 0);
    post_shinden_json(
        &state.0,
        "https://lista.shinden.pl/api/title-watched-episodes-change",
        &payload,
        "mark_episode_unwatched",
    )
    .await
}

#[tauri::command]
fn get_watching_cache_refresh_status(
    state: tauri::State<'_, Api>,
) -> Result<WatchingCacheRefreshStatus, String> {
    refresh_status_snapshot(&state.1)
}

#[tauri::command]
async fn refresh_watching_anime_cache(
    state: tauri::State<'_, Api>,
    filter: Option<WatchingAnimeFilter>,
    force: Option<bool>,
) -> Result<WatchingCacheRefreshSummary, String> {
    let filter = filter.unwrap_or_default();
    let force = force.unwrap_or(false);

    if let Some(summary) = begin_watching_cache_refresh(&state.1)? {
        return Ok(summary);
    }

    let refresh_result =
        refresh_watching_cache_inner(&state.0, &state.1, &state.2, &filter, force).await;

    match refresh_result {
        Ok(status) => Ok(WatchingCacheRefreshSummary {
            status,
            already_running: false,
        }),
        Err(error) => {
            update_refresh_status(&state.1, |status| {
                status.running = false;
                status.current_title.clear();
                status.last_finished_at_ms = Some(unix_timestamp_ms_u64());
                status.last_error = Some(error.clone());
            })?;
            Err(error)
        }
    }
}

#[tauri::command]
async fn refresh_watching_anime_cache_item(
    state: tauri::State<'_, Api>,
    title_id: u64,
    filter: Option<WatchingAnimeFilter>,
    force: Option<bool>,
) -> Result<WatchingCacheRefreshSummary, String> {
    let filter = filter.unwrap_or_default();
    let force = force.unwrap_or(false);

    if let Some(summary) = begin_watching_cache_refresh(&state.1)? {
        return Ok(summary);
    }

    let refresh_result =
        refresh_watching_cache_item_inner(&state.0, &state.1, &state.2, title_id, &filter, force)
            .await;

    match refresh_result {
        Ok(status) => Ok(WatchingCacheRefreshSummary {
            status,
            already_running: false,
        }),
        Err(error) => {
            update_refresh_status(&state.1, |status| {
                status.running = false;
                status.current_title.clear();
                status.last_finished_at_ms = Some(unix_timestamp_ms_u64());
                status.last_error = Some(error.clone());
            })?;
            Err(error)
        }
    }
}

async fn fetch_all_watching_items(
    api: &ShindenAPI,
    user_id_cache: &Mutex<CachedUserId>,
) -> Result<Vec<WatchingListApiItem>, String> {
    let user_id = fetch_current_user_id_cached(api, user_id_cache, "get_watching_anime").await?;
    fetch_all_watching_items_for_status(api, &user_id, "in progress").await
}

async fn fetch_all_userlist_items(
    api: &ShindenAPI,
    user_id_cache: &Mutex<CachedUserId>,
) -> Result<Vec<WatchingListApiItem>, String> {
    let user_id = fetch_current_user_id_cached(api, user_id_cache, "search").await?;
    let mut items = Vec::new();

    for status in WATCHING_LIST_STATUSES {
        items.extend(fetch_all_watching_items_for_status(api, &user_id, status).await?);
    }

    Ok(items)
}

async fn fetch_all_watching_items_for_status(
    api: &ShindenAPI,
    user_id: &str,
    status: &str,
) -> Result<Vec<WatchingListApiItem>, String> {
    let mut offset = 0;
    let mut items = Vec::new();

    loop {
        let page =
            fetch_watching_list_status_page(api, user_id, status, WATCHING_LIST_PAGE_LIMIT, offset)
                .await?;
        let loaded = page.items.len();
        let total = page.count;

        items.extend(page.items);

        offset += loaded;
        if loaded == 0 || offset >= total {
            break;
        }
    }

    Ok(items)
}

async fn fetch_current_user_id(api: &ShindenAPI, context: &str) -> Result<String, String> {
    let profile_context = format!("{context} profile");
    let profile_html = api
        .get_html("https://shinden.pl/user")
        .await
        .map_err(|e| command_error(&profile_context, e))?;

    extract_user_id_from_profile_html(&profile_html)
        .ok_or_else(|| command_error(&profile_context, "User is not logged in"))
}

async fn fetch_current_user_id_cached(
    api: &ShindenAPI,
    user_id_cache: &Mutex<CachedUserId>,
    context: &str,
) -> Result<String, String> {
    let now_ms = unix_timestamp_ms_u64();
    {
        let cache = lock_user_id_cache(user_id_cache)?;
        if let Some(user_id) = cached_user_id_if_fresh(&*cache, now_ms) {
            return Ok(user_id);
        }
    }

    match fetch_current_user_id(api, context).await {
        Ok(user_id) => {
            store_cached_user_id(user_id_cache, &user_id, now_ms)?;
            Ok(user_id)
        }
        Err(error) => {
            if is_transient_user_profile_error(&error) {
                let cached_user_id = {
                    let cache = lock_user_id_cache(user_id_cache)?;
                    cache.user_id.clone()
                };
                if let Some(user_id) = cached_user_id {
                    let _ = append_project_log(
                        "WARNING",
                        &format!("Using cached Shinden user id after {context} profile error: {error}"),
                    );
                    return Ok(user_id);
                }
            }

            Err(error)
        }
    }
}

fn is_transient_user_profile_error(error: &str) -> bool {
    error.contains("429 Too Many Requests") || error.contains("error sending request")
}

fn cached_user_id_if_fresh(cache: &CachedUserId, now_ms: u64) -> Option<String> {
    let user_id = cache.user_id.as_ref()?;
    if now_ms.saturating_sub(cache.checked_at_ms) <= USER_ID_CACHE_TTL_MS {
        Some(user_id.clone())
    } else {
        None
    }
}

fn store_cached_user_id(
    user_id_cache: &Mutex<CachedUserId>,
    user_id: &str,
    checked_at_ms: u64,
) -> Result<(), String> {
    let mut cache = lock_user_id_cache(user_id_cache)?;
    store_cached_user_id_value(&mut cache, user_id, checked_at_ms);
    Ok(())
}

fn store_cached_user_id_value(cache: &mut CachedUserId, user_id: &str, checked_at_ms: u64) {
    cache.user_id = Some(user_id.to_string());
    cache.checked_at_ms = checked_at_ms;
}

fn clear_cached_user_id(user_id_cache: &Mutex<CachedUserId>) -> Result<(), String> {
    let mut cache = lock_user_id_cache(user_id_cache)?;
    *cache = CachedUserId::default();
    Ok(())
}

fn lock_user_id_cache(
    user_id_cache: &Mutex<CachedUserId>,
) -> Result<std::sync::MutexGuard<'_, CachedUserId>, String> {
    user_id_cache
        .lock()
        .map_err(|_| command_error("user_id_cache", "User id cache lock poisoned"))
}

async fn fetch_shinden_basic_auth(api: &ShindenAPI) -> Result<String, String> {
    let profile_html = api
        .get_html("https://shinden.pl/user")
        .await
        .map_err(|e| command_error("legacy auth profile", e))?;

    extract_shinden_basic_auth(&profile_html)
        .ok_or_else(|| command_error("legacy auth profile", "Could not find Shinden auth token"))
}

#[tauri::command]
async fn login(
    state: tauri::State<'_, Api>,
    username: String,
    password: String,
) -> Result<(), String> {
    let result = state
        .0
        .login(&username, &password)
        .await
        .map_err(|e| command_error("login", e));

    if result.is_ok() {
        clear_cached_user_id(&state.2)?;
    }

    result
}

#[tauri::command]
async fn logout(state: tauri::State<'_, Api>) -> Result<(), String> {
    state.0.logout().await.map_err(|e| command_error("logout", e))?;
    clear_cached_user_id(&state.2)
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

async fn fetch_watching_list_status_page(
    api: &ShindenAPI,
    user_id: &str,
    status: &str,
    limit: usize,
    offset: usize,
) -> Result<WatchingListApiResult, String> {
    let url = watching_list_status_url(user_id, status, limit, offset);
    let response = api
        .client
        .get(&url)
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| command_error("get_watching_anime request", e))?
        .error_for_status()
        .map_err(|e| command_error("get_watching_anime response", e))?;
    let payload = response
        .json::<WatchingListApiResponse>()
        .await
        .map_err(|e| command_error("get_watching_anime json", e))?;

    if !payload.success {
        return Err(command_error(
            "get_watching_anime json",
            "List API returned success=false",
        ));
    }

    Ok(payload.result)
}

async fn fetch_title_status(
    api: &ShindenAPI,
    title_id: u64,
    user_id: &str,
) -> Result<Option<TitleStatusApiTitle>, String> {
    let response = api
        .client
        .get(title_status_url(title_id, user_id))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| command_error("title_status request", e))?
        .error_for_status()
        .map_err(|e| command_error("title_status response", e))?;

    let payload = response
        .json::<TitleStatusApiResponse>()
        .await
        .map_err(|e| command_error("title_status json", e))?;

    if !payload.success {
        return Err(command_error(
            "title_status json",
            payload
                .message
                .unwrap_or_else(|| "List API returned success=false".to_string()),
        ));
    }

    Ok(payload.result.title)
}

async fn fetch_title_episode_progress(
    api: &ShindenAPI,
    title_id: u64,
    user_id: &str,
) -> Result<Vec<TitleEpisodeApiItem>, String> {
    let response = api
        .client
        .get(title_episodes_url(title_id, user_id))
        .header(ACCEPT, "application/json")
        .send()
        .await
        .map_err(|e| command_error("title_episodes request", e))?
        .error_for_status()
        .map_err(|e| command_error("title_episodes response", e))?;

    let payload = response
        .json::<TitleEpisodesApiResponse>()
        .await
        .map_err(|e| command_error("title_episodes json", e))?;

    if !payload.success {
        return Err(command_error(
            "title_episodes json",
            payload
                .message
                .unwrap_or_else(|| "List API returned success=false".to_string()),
        ));
    }

    Ok(payload.result.items)
}

async fn post_shinden_json<T: Serialize>(
    api: &ShindenAPI,
    url: &str,
    payload: &T,
    context: &str,
) -> Result<(), String> {
    let request_context = format!("{context} request");
    let response_context = format!("{context} response");
    let json_context = format!("{context} json");
    let response = api
        .client
        .post(url)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header(ORIGIN, "https://lista.shinden.pl")
        .header(REFERER, "https://lista.shinden.pl/")
        .json(payload)
        .send()
        .await
        .map_err(|e| command_error(&request_context, e))?
        .error_for_status()
        .map_err(|e| command_error(&response_context, e))?;

    let payload = response
        .json::<ShindenWriteResponse>()
        .await
        .map_err(|e| command_error(&json_context, e))?;

    if payload.success {
        Ok(())
    } else {
        Err(command_error(
            &json_context,
            payload
                .message
                .unwrap_or_else(|| "Shinden returned success=false".to_string()),
        ))
    }
}

async fn post_legacy_anime_status(
    api: &ShindenAPI,
    title_id: u64,
    user_id: &str,
    status: Option<&str>,
    input: &TitleStatusChangeInput,
) -> Result<(), String> {
    let legacy_statuses = shinden_legacy_watch_status_values(status)?;
    if legacy_statuses.is_empty() {
        return post_legacy_anime_status_delete(api, title_id, user_id).await;
    }

    let basic_auth = fetch_shinden_basic_auth(api).await?;
    let priority = input.priority.to_string();
    let recommend = input.recommend.to_string();
    let url = legacy_userlist_series_url(user_id, title_id);

    let mut last_error = None;

    for legacy_status in legacy_statuses {
        let response = api
            .client
            .post(&url)
            .header(ACCEPT, "application/json")
            .header("X-Requested-With", "XMLHttpRequest")
            .header(ORIGIN, "https://shinden.pl")
            .header(REFERER, series_url(title_id))
            .form(&[
                ("status", legacy_status),
                ("priority", priority.as_str()),
                ("recommend", recommend.as_str()),
                ("auth", basic_auth.as_str()),
            ])
            .send()
            .await
            .map_err(|e| command_error("legacy_update_anime_status request", e))?
            .error_for_status()
            .map_err(|e| command_error("legacy_update_anime_status response", e))?;

        if let Err(error) = validate_legacy_write_response(response.text().await.map_err(|e| {
            command_error("legacy_update_anime_status text", e)
        })?) {
            last_error = Some(error);
            continue;
        }

        match verify_title_status_change_with_user(
            api,
            title_id,
            user_id,
            status,
            "legacy_update_anime_status",
        )
        .await
        {
            Ok(()) => return Ok(()),
            Err(error) => last_error = Some(error),
        }
    }

    Err(last_error.unwrap_or_else(|| {
        command_error(
            "legacy_update_anime_status",
            "Shinden did not confirm legacy status change",
        )
    }))
}

async fn post_legacy_anime_status_delete(
    api: &ShindenAPI,
    title_id: u64,
    user_id: &str,
) -> Result<(), String> {
    let basic_auth = fetch_shinden_basic_auth(api).await?;
    let url = legacy_userlist_series_url(user_id, title_id);
    let response = api
        .client
        .post(&url)
        .header(ACCEPT, "application/json")
        .header(CONTENT_TYPE, "application/json")
        .header("X-HTTP-Method-Override", "DELETE")
        .header("X-Requested-With", "XMLHttpRequest")
        .header(ORIGIN, "https://shinden.pl")
        .header(REFERER, series_url(title_id))
        .json(&serde_json::json!({ "auth": basic_auth }))
        .send()
        .await
        .map_err(|e| command_error("legacy_delete_anime_status request", e))?
        .error_for_status()
        .map_err(|e| command_error("legacy_delete_anime_status response", e))?;

    validate_legacy_write_response(response.text().await.map_err(|e| {
        command_error("legacy_delete_anime_status text", e)
    })?)
}

fn validate_legacy_write_response(response_text: String) -> Result<(), String> {
    let trimmed = response_text.trim();
    if trimmed.is_empty() {
        return Ok(());
    }

    let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) else {
        return Ok(());
    };

    if value
        .get("success")
        .and_then(|success| success.as_bool())
        .is_some_and(|success| !success)
    {
        return Err(command_error(
            "legacy_update_anime_status json",
            value
                .get("message")
                .and_then(|message| message.as_str())
                .unwrap_or("Shinden returned success=false"),
        ));
    }

    if let Some(error) = value
        .get("error")
        .or_else(|| value.get("err"))
        .and_then(|error| error.as_str())
    {
        if !error.trim().is_empty() {
            return Err(command_error("legacy_update_anime_status json", error));
        }
    }

    Ok(())
}

async fn verify_title_status_change_with_user(
    api: &ShindenAPI,
    title_id: u64,
    user_id: &str,
    status: Option<&str>,
    context: &str,
) -> Result<(), String> {
    let expected_status = shinden_watch_status_value(status)?;
    let statuses = match expected_status {
        Some(status) => vec![status],
        None => WATCHING_LIST_STATUSES.to_vec(),
    };

    let mut found_status = None;

    for status in statuses {
        let items = fetch_all_watching_items_for_status(api, &user_id, status).await?;

        if let Some(item) = items.into_iter().find(|item| item.title_id == title_id) {
            found_status = Some(item.watch_status.unwrap_or_else(|| status.to_string()));
            break;
        }
    }

    match expected_status {
        Some(expected_status) => {
            let Some(found_status) = found_status else {
                return Err(command_error(
                    context,
                    format!("Shinden did not confirm status change for title {title_id}"),
                ));
            };

            if title_status_matches(Some(&found_status), Some(expected_status))? {
                Ok(())
            } else {
                Err(command_error(
                    context,
                    format!(
                        "Shinden saved status {found_status}, expected {expected_status} for title {title_id}"
                    ),
                ))
            }
        }
        None => {
            if found_status.is_none() {
                Ok(())
            } else {
                Err(command_error(
                    context,
                    format!("Shinden did not remove title {title_id} from the user list"),
                ))
            }
        }
    }
}

async fn refresh_watching_cache_inner(
    api: &ShindenAPI,
    status: &Mutex<WatchingCacheRefreshStatus>,
    user_id_cache: &Mutex<CachedUserId>,
    filter: &WatchingAnimeFilter,
    force: bool,
) -> Result<WatchingCacheRefreshStatus, String> {
    let items = fetch_all_watching_items(api, user_id_cache).await?;
    let mut cache = load_watching_availability_cache();
    let subtitle_key = selected_subtitle_language_key(filter);
    let subtitle_cache_key = selected_subtitle_cache_key(filter);
    let now_ms = unix_timestamp_ms_u64();

    update_refresh_status(status, |status| {
        status.total = items.len();
    })?;

    let plan = collect_watching_cache_refresh_plan(
        &items,
        &cache,
        subtitle_cache_key.as_deref(),
        now_ms,
        force,
    );

    update_refresh_status(status, |status| {
        status.current = plan.processed;
        status.skipped = plan.skipped;
    })?;

    let subtitle_key = subtitle_key.as_deref();
    let subtitle_cache_key = subtitle_cache_key.as_deref();
    let exclude_ai_subtitles = filter.exclude_ai_subtitles();
    let mut scan_results = stream::iter(plan.items_to_scan.into_iter().map(|item| async move {
        let cache_key = watching_cache_key(item.title_id);
        let item_title = item.title.clone();
        let result = scan_watching_item_availability(
            api,
            &item,
            subtitle_key,
            subtitle_cache_key,
            exclude_ai_subtitles,
        )
        .await;

        (cache_key, item_title, result)
    }))
    .buffer_unordered(WATCHING_CACHE_REFRESH_CONCURRENCY);

    while let Some((cache_key, item_title, scan_result)) = scan_results.next().await {
        update_refresh_status(status, |status| {
            status.current += 1;
            status.current_title = item_title.clone();
        })?;

        match scan_result {
            Ok(entry) => {
                cache.entries.insert(cache_key, entry);
                save_watching_availability_cache(&cache)?;
                update_refresh_status(status, |status| {
                    status.refreshed += 1;
                })?;
            }
            Err(error) => {
                let visible_error = watching_cache_item_error_message(&item_title);
                let _ = command_error("watching_cache item", format!("{visible_error}: {error}"));
                update_refresh_status(status, |status| {
                    status.failed += 1;
                    status.last_error = Some(visible_error);
                })?;
            }
        }
    }

    update_refresh_status(status, |status| {
        status.running = false;
        status.current_title.clear();
        status.last_finished_at_ms = Some(unix_timestamp_ms_u64());
    })?;

    refresh_status_snapshot(status)
}

async fn refresh_watching_cache_item_inner(
    api: &ShindenAPI,
    status: &Mutex<WatchingCacheRefreshStatus>,
    user_id_cache: &Mutex<CachedUserId>,
    title_id: u64,
    filter: &WatchingAnimeFilter,
    force: bool,
) -> Result<WatchingCacheRefreshStatus, String> {
    let items = fetch_all_watching_items(api, user_id_cache).await?;
    let item = items.into_iter().find(|item| item.title_id == title_id);
    let cache_key = watching_cache_key(title_id);
    let mut cache = load_watching_availability_cache();

    update_refresh_status(status, |status| {
        status.total = 1;
        status.current_title = item
            .as_ref()
            .map(|item| item.title.clone())
            .unwrap_or_else(|| format!("Anime {title_id}"));
    })?;

    let Some(item) = item else {
        let removed = cache.entries.remove(&cache_key).is_some();
        if removed {
            save_watching_availability_cache(&cache)?;
        }
        return finish_single_watching_cache_refresh(status, removed);
    };

    if !has_unwatched_episodes(&item) {
        let removed = cache.entries.remove(&cache_key).is_some();
        if removed {
            save_watching_availability_cache(&cache)?;
        }
        return finish_single_watching_cache_refresh(status, removed);
    }

    let subtitle_key = selected_subtitle_language_key(filter);
    let subtitle_cache_key = selected_subtitle_cache_key(filter);

    if !force
        && cache.entries.get(&cache_key).is_some_and(|entry| {
            cache_entry_satisfies_refresh(
                entry,
                &item,
                subtitle_cache_key.as_deref(),
                unix_timestamp_ms_u64(),
                false,
            )
        })
    {
        return finish_single_watching_cache_refresh(status, false);
    }

    match scan_watching_item_availability(
        api,
        &item,
        subtitle_key.as_deref(),
        subtitle_cache_key.as_deref(),
        filter.exclude_ai_subtitles(),
    )
    .await
    {
        Ok(entry) => {
            cache.entries.insert(cache_key, entry);
            save_watching_availability_cache(&cache)?;
            finish_single_watching_cache_refresh(status, true)
        }
        Err(error) => {
            let visible_error = watching_cache_item_error_message(&item.title);
            let _ = command_error("watching_cache item", format!("{visible_error}: {error}"));
            update_refresh_status(status, |status| {
                status.current = 1;
                status.failed = 1;
                status.last_error = Some(visible_error.clone());
            })?;
            Err(visible_error)
        }
    }
}

fn finish_single_watching_cache_refresh(
    status: &Mutex<WatchingCacheRefreshStatus>,
    refreshed: bool,
) -> Result<WatchingCacheRefreshStatus, String> {
    update_refresh_status(status, |status| {
        status.current = 1;
        if refreshed {
            status.refreshed = 1;
        } else {
            status.skipped = 1;
        }
        status.running = false;
        status.current_title.clear();
        status.last_finished_at_ms = Some(unix_timestamp_ms_u64());
    })?;

    refresh_status_snapshot(status)
}

fn collect_watching_cache_refresh_plan(
    items: &[WatchingListApiItem],
    cache: &WatchingAvailabilityCache,
    subtitle_cache_key: Option<&str>,
    now_ms: u64,
    force: bool,
) -> WatchingCacheRefreshPlan {
    let mut items_to_scan = Vec::new();
    let mut skipped = 0;

    for item in items {
        if !has_unwatched_episodes(item) {
            skipped += 1;
            continue;
        }

        let cache_key = watching_cache_key(item.title_id);
        if cache.entries.get(&cache_key).is_some_and(|entry| {
            cache_entry_satisfies_refresh(entry, item, subtitle_cache_key, now_ms, force)
        }) {
            skipped += 1;
            continue;
        }

        items_to_scan.push(item.clone());
    }

    WatchingCacheRefreshPlan {
        items_to_scan,
        skipped,
        processed: skipped,
    }
}

async fn scan_watching_item_availability(
    api: &ShindenAPI,
    item: &WatchingListApiItem,
    subtitle_key: Option<&str>,
    subtitle_cache_key: Option<&str>,
    _exclude_ai_subtitles: bool,
) -> Result<WatchingAvailabilityCacheEntry, String> {
    let series_url = series_url(item.title_id);
    let episodes = get_watching_cache_episodes(api, &series_url).await?;
    let watched_count = watched_episode_count(item) as usize;
    let mut has_available_unwatched_episode = false;
    let mut subtitle_availability = HashMap::new();

    for episode in episodes.into_iter().skip(watched_count) {
        let players = get_watching_cache_players(api, &episode.link).await?;

        if record_watching_cache_episode_availability(
            &players,
            subtitle_key,
            &mut subtitle_availability,
        ) {
            has_available_unwatched_episode = true;
            break;
        }
    }

    if let Some(cache_key) = subtitle_cache_key {
        subtitle_availability
            .entry(cache_key.to_string())
            .or_insert(false);
    }

    Ok(WatchingAvailabilityCacheEntry {
        title_id: item.title_id,
        watched_episodes_cnt: watched_episode_count(item),
        total_episodes: item.episodes,
        has_available_unwatched_episode,
        subtitle_availability,
        checked_at_ms: unix_timestamp_ms_u64(),
    })
}

fn record_watching_cache_episode_availability(
    players: &[Player],
    subtitle_key: Option<&str>,
    subtitle_availability: &mut HashMap<String, bool>,
) -> bool {
    record_watching_cache_episode_subtitle_availability(
        players.iter().map(|player| player.lang_subs.as_str()),
        subtitle_key,
        subtitle_availability,
    )
}

fn record_watching_cache_episode_subtitle_availability<'a, I>(
    player_subtitles: I,
    subtitle_key: Option<&str>,
    subtitle_availability: &mut HashMap<String, bool>,
) -> bool
where
    I: IntoIterator<Item = &'a str>,
{
    let mut has_players = false;

    for language in player_subtitles {
        has_players = true;
        if subtitle_key.is_some() {
            record_subtitle_language_availability(subtitle_availability, language);
        }
    }

    has_players
}

async fn get_watching_cache_episodes(
    api: &ShindenAPI,
    series_url: &str,
) -> Result<Vec<Episode>, String> {
    let mut last_error = String::new();

    for attempt in 0..=WATCHING_CACHE_REQUEST_RETRIES {
        match api.get_episodes(series_url).await {
            Ok(episodes) => return Ok(episodes),
            Err(error) => {
                last_error = error.to_string();
                log_watching_cache_retry("episodes", series_url, attempt, &last_error);
                wait_before_watching_cache_retry(attempt);
            }
        }
    }

    Err(format!(
        "Nie udalo sie pobrac listy odcinkow: {last_error}"
    ))
}

async fn get_watching_cache_players(
    api: &ShindenAPI,
    episode_url: &str,
) -> Result<Vec<Player>, String> {
    let mut last_error = String::new();

    for attempt in 0..=WATCHING_CACHE_REQUEST_RETRIES {
        match api.get_players(episode_url).await {
            Ok(players) => return Ok(players),
            Err(error) => {
                last_error = error.to_string();
                log_watching_cache_retry("players", episode_url, attempt, &last_error);
                wait_before_watching_cache_retry(attempt);
            }
        }
    }

    Err(format!("Nie udalo sie sprawdzic odcinka: {last_error}"))
}

fn wait_before_watching_cache_retry(attempt: usize) {
    if attempt < WATCHING_CACHE_REQUEST_RETRIES {
        std::thread::sleep(Duration::from_millis(WATCHING_CACHE_RETRY_DELAY_MS));
    }
}

fn log_watching_cache_retry(context: &str, url: &str, attempt: usize, error: &str) {
    let _ = append_project_log(
        "WARNING",
        &format!(
            "watching_cache {context} attempt {}/{} failed for {url}: {error}",
            attempt + 1,
            WATCHING_CACHE_REQUEST_RETRIES + 1
        ),
    );
}

fn watching_cache_item_error_message(title: &str) -> String {
    format!("Nie udalo sie sprawdzic: {title}")
}

fn watching_list_status_url(user_id: &str, status: &str, limit: usize, offset: usize) -> String {
    let status = watch_status_list_slug(status);

    format!(
        "https://lista.shinden.pl/api/userlist/{user_id}/anime/{status}?limit={limit}&offset={offset}"
    )
}

fn title_status_url(title_id: u64, user_id: &str) -> String {
    format!("https://lista.shinden.pl/api/title-status/{title_id}/{user_id}")
}

fn legacy_userlist_series_url(user_id: &str, title_id: u64) -> String {
    format!("https://shinden.pl/api/userlist/{user_id}/series/{title_id}")
}

fn series_url(title_id: u64) -> String {
    format!("https://shinden.pl/series/{title_id}")
}

fn title_id_from_series_url(url: &str) -> Option<String> {
    ["/series/", "/titles/"]
        .iter()
        .find_map(|marker| extract_ascii_digits_after(url, marker))
}

fn title_episodes_url(title_id: u64, user_id: &str) -> String {
    format!("https://lista.shinden.pl/api/title-episodes/{title_id}/{user_id}")
}

fn is_true_final_episode(episode_no: u32, total_episodes: Option<u32>) -> bool {
    total_episodes
        .map(|total| total > 0 && episode_no == total)
        .unwrap_or(false)
}

fn merge_episode_progress(
    playback_episodes: Vec<Episode>,
    progress_episodes: Vec<TitleEpisodeApiItem>,
    total_episodes: Option<u32>,
) -> Vec<EpisodeProgress> {
    let progress_by_number: HashMap<u32, TitleEpisodeApiItem> = progress_episodes
        .into_iter()
        .map(|episode| (episode.episode_no, episode))
        .collect();

    playback_episodes
        .into_iter()
        .enumerate()
        .map(|(index, episode)| {
            let fallback_episode_no = (index + 1).min(u32::MAX as usize) as u32;
            let progress = progress_by_number.get(&fallback_episode_no);
            let episode_no = progress
                .map(|progress| progress.episode_no)
                .unwrap_or(fallback_episode_no);
            let watched = progress.and_then(|progress| progress.watched.as_ref());

            EpisodeProgress {
                title: episode.title,
                link: episode.link,
                episode_id: progress.map(|progress| progress.episode_id),
                episode_no,
                watched: watched.is_some(),
                view_count: watched.map(|watched| watched.view_cnt).unwrap_or_default(),
                total_episodes,
                is_true_final_episode: is_true_final_episode(episode_no, total_episodes),
            }
        })
        .collect()
}

fn extract_user_id_from_profile_html(html: &str) -> Option<String> {
    ["https://lista.shinden.pl/animelist/", "/animelist/"]
        .iter()
        .find_map(|marker| extract_ascii_digits_after(html, marker))
}

fn extract_shinden_basic_auth(html: &str) -> Option<String> {
    [
        "_Storage.basic = \"",
        "_Storage.basic=\"",
        "_Storage.basic = '",
        "_Storage.basic='",
        "\"basic\":\"",
        "'basic':'",
        "basic: \"",
        "basic:\"",
        "basic: '",
        "basic:'",
    ]
    .iter()
    .find_map(|marker| extract_until_quote_after(html, marker))
    .filter(|token| !token.trim().is_empty())
}

fn extract_ascii_digits_after(source: &str, marker: &str) -> Option<String> {
    let start = source.find(marker)? + marker.len();
    let digits: String = source[start..]
        .chars()
        .take_while(|character| character.is_ascii_digit())
        .collect();

    if digits.is_empty() {
        None
    } else {
        Some(digits)
    }
}

fn extract_until_quote_after(source: &str, marker: &str) -> Option<String> {
    let start = source.find(marker)? + marker.len();
    let quote = marker.chars().last()?;
    let value = source[start..].split(quote).next()?.trim();

    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

fn shinden_watch_status_value(status: Option<&str>) -> Result<Option<&'static str>, String> {
    let Some(status) = status else {
        return Ok(None);
    };

    let normalized = status.trim().to_ascii_lowercase().replace('_', " ");
    match normalized.as_str() {
        "" | "no" | "none" | "null" => Ok(None),
        "in progress" | "in-progress" | "inprogress" | "watching" | "ogladam" => {
            Ok(Some("in progress"))
        }
        "completed" | "obejrzane" => Ok(Some("completed")),
        "skip" | "pomijam" => Ok(Some("skip")),
        "hold" | "wstrzymane" => Ok(Some("hold")),
        "dropped" | "porzucone" => Ok(Some("dropped")),
        "plan" | "planuje" => Ok(Some("plan")),
        _ => Err(format!("Unsupported anime status: {status}")),
    }
}

fn shinden_legacy_watch_status_values(status: Option<&str>) -> Result<Vec<&'static str>, String> {
    Ok(match shinden_watch_status_value(status)? {
        Some("in progress") => vec!["in progress", "in-progress", "watching"],
        Some("completed") => vec!["completed", "watched"],
        Some("plan") => vec!["plan", "planned", "to-watch"],
        Some("dropped") => vec!["dropped"],
        Some("hold") => vec!["hold", "on-hold"],
        Some("skip") => vec!["skip", "skipped"],
        Some(status) => vec![status],
        None => Vec::new(),
    })
}

fn title_status_matches(found: Option<&str>, expected: Option<&str>) -> Result<bool, String> {
    Ok(shinden_watch_status_value(found)? == shinden_watch_status_value(expected)?)
}

fn watch_status_list_slug(status: &str) -> &'static str {
    match status.trim().to_ascii_lowercase().as_str() {
        "in progress" | "in-progress" | "inprogress" => "in-progress",
        "completed" => "completed",
        "skip" => "skip",
        "hold" => "hold",
        "dropped" => "dropped",
        "plan" => "plan",
        _ => "in-progress",
    }
}

fn build_title_status_payload(
    title_id: u64,
    status: Option<&str>,
    is_favourite: Option<u8>,
) -> Result<TitleStatusChangePayload, String> {
    build_title_status_payload_with_details(title_id, status, is_favourite, 0, 0)
}

fn build_title_status_payload_with_details(
    title_id: u64,
    status: Option<&str>,
    is_favourite: Option<u8>,
    priority: i32,
    recommend: i32,
) -> Result<TitleStatusChangePayload, String> {
    Ok(TitleStatusChangePayload {
        input: vec![TitleStatusChangeInput {
            title_id,
            watch_status: shinden_watch_status_value(status)?,
            is_favourite: is_favourite.unwrap_or_default(),
            priority,
            recommend,
        }],
    })
}

fn build_watched_episode_payload(
    title_id: u64,
    episode_id: u64,
    created_time: String,
    view_count: u32,
) -> WatchedEpisodesChangePayload {
    WatchedEpisodesChangePayload {
        title_id,
        episodes: vec![WatchedEpisodeChangeInput {
            episode_id,
            view_cnt: view_count,
            created_time,
        }],
    }
}

fn map_watching_list_item_details(item: WatchingListApiItem) -> Option<WatchingAnime> {
    let name = item.title.trim().to_string();
    if name.is_empty() {
        return None;
    }

    let watched_episodes_count = watched_episode_count(&item);
    let watch_status = item
        .watch_status
        .as_deref()
        .unwrap_or("in progress")
        .to_string();

    Some(WatchingAnime {
        title_id: item.title_id,
        name,
        url: series_url(item.title_id),
        image_url: item
            .cover_id
            .map(|cover_id| format!("https://cdn.shinden.eu/cdn1/images/genuine/{cover_id}.jpg"))
            .unwrap_or_else(|| SHINDEN_TITLE_PLACEHOLDER.to_string()),
        anime_type: item.anime_type.unwrap_or_default(),
        rating: format_rating(item.summary_rating_total.as_deref()),
        episodes: format_episode_progress(item.watched_episodes_cnt.as_deref(), item.episodes),
        description: item.description_pl.or(item.description_en).unwrap_or_default(),
        watch_status,
        is_favourite: item.is_favourite.unwrap_or_default(),
        watched_episodes_count,
        total_episodes: item.episodes,
    })
}

fn map_watching_list_item(item: WatchingListApiItem) -> Option<Anime> {
    map_watching_list_item_details(item).map(|item| Anime {
        name: item.name,
        url: item.url,
        image_url: item.image_url,
        anime_type: item.anime_type,
        rating: item.rating,
        episodes: item.episodes,
        description: item.description,
    })
}

fn map_search_anime_results(
    results: Vec<Anime>,
    watching_items: Vec<WatchingListApiItem>,
) -> Vec<SearchAnime> {
    let watching_by_title_id: HashMap<u64, WatchingListApiItem> = watching_items
        .into_iter()
        .map(|item| (item.title_id, item))
        .collect();

    results
        .into_iter()
        .map(|anime| map_search_anime_details(anime, &watching_by_title_id))
        .collect()
}

fn map_search_anime_details(
    anime: Anime,
    watching_by_title_id: &HashMap<u64, WatchingListApiItem>,
) -> SearchAnime {
    let title_id = title_id_from_series_url(&anime.url).and_then(|value| value.parse::<u64>().ok());
    let watching_item = title_id.and_then(|title_id| watching_by_title_id.get(&title_id));

    SearchAnime {
        anime,
        title_id,
        watch_status: watching_item
            .and_then(|item| item.watch_status.clone())
            .unwrap_or_else(|| "no".to_string()),
        is_favourite: watching_item
            .and_then(|item| item.is_favourite)
            .unwrap_or_default(),
        total_episodes: watching_item.and_then(|item| item.episodes),
    }
}

fn has_unwatched_episodes(item: &WatchingListApiItem) -> bool {
    match item.episodes {
        Some(total) => watched_episode_count(item) < total,
        None => true,
    }
}

fn watching_progress_filter_matches(
    item: &WatchingListApiItem,
    filter: &WatchingAnimeFilter,
) -> bool {
    !filter.only_available_unwatched() || has_unwatched_episodes(item)
}

fn watching_cache_filter_matches(
    item: &WatchingListApiItem,
    filter: &WatchingAnimeFilter,
    cache: &WatchingAvailabilityCache,
) -> bool {
    if !watching_progress_filter_matches(item, filter) {
        return false;
    }

    if !filter.only_available_unwatched() {
        return true;
    }

    let Some(entry) = cache.entries.get(&watching_cache_key(item.title_id)) else {
        return false;
    };

    if !cache_entry_matches_item(entry, item) || !entry.has_available_unwatched_episode {
        return false;
    }

    selected_subtitle_cache_key(filter)
        .map(|key| {
            entry
                .subtitle_availability
                .get(&key)
                .copied()
                .unwrap_or(false)
        })
        .unwrap_or(true)
}

fn cache_entry_matches_item(
    entry: &WatchingAvailabilityCacheEntry,
    item: &WatchingListApiItem,
) -> bool {
    entry.title_id == item.title_id
        && entry.watched_episodes_cnt == watched_episode_count(item)
        && entry.total_episodes == item.episodes
}

fn cache_entry_satisfies_refresh(
    entry: &WatchingAvailabilityCacheEntry,
    item: &WatchingListApiItem,
    subtitle_key: Option<&str>,
    now_ms: u64,
    force: bool,
) -> bool {
    if force || !cache_entry_matches_item(entry, item) {
        return false;
    }

    if now_ms.saturating_sub(entry.checked_at_ms) > WATCHING_CACHE_TTL_MS {
        return false;
    }

    subtitle_key
        .map(|key| entry.subtitle_availability.contains_key(key))
        .unwrap_or(true)
}

fn selected_subtitle_language_key(filter: &WatchingAnimeFilter) -> Option<String> {
    if !filter.check_subtitle_availability_online() {
        return None;
    }

    let key = subtitle_language_key(filter.subtitle_language());
    if key.is_empty() || key == "any" {
        None
    } else {
        Some(key)
    }
}

fn selected_subtitle_cache_key(filter: &WatchingAnimeFilter) -> Option<String> {
    selected_subtitle_language_key(filter).map(|key| {
        if filter.exclude_ai_subtitles() {
            format!("{key}:human")
        } else {
            key
        }
    })
}

fn record_subtitle_language_availability(
    subtitle_availability: &mut HashMap<String, bool>,
    language: &str,
) {
    let key = subtitle_language_key(language);
    if key.is_empty() || key == "any" {
        return;
    }

    subtitle_availability.insert(key.clone(), true);
    if !is_ai_subtitle_language(language, &key) {
        subtitle_availability.insert(format!("{key}:human"), true);
    }
}

fn watching_cache_key(title_id: u64) -> String {
    title_id.to_string()
}

fn watched_episode_count(item: &WatchingListApiItem) -> u32 {
    item.watched_episodes_cnt
        .as_deref()
        .and_then(|watched| watched.trim().parse::<u32>().ok())
        .unwrap_or_default()
}

fn subtitle_language_matches(player_lang_subs: &str, selected_language: &str) -> bool {
    subtitle_language_matches_with_options(player_lang_subs, selected_language, false)
}

fn subtitle_language_matches_with_options(
    player_lang_subs: &str,
    selected_language: &str,
    exclude_ai_subtitles: bool,
) -> bool {
    let selected_language = selected_language.trim();
    if selected_language.is_empty() {
        return true;
    }

    let selected_key = subtitle_language_key(selected_language);
    if selected_key == "any" {
        return true;
    }

    if exclude_ai_subtitles && is_ai_subtitle_language(player_lang_subs, &selected_key) {
        return false;
    }

    let player_key = subtitle_language_key(player_lang_subs);
    player_key == selected_key
}

fn subtitle_language_key(language: &str) -> String {
    let language = language.trim().to_ascii_lowercase();

    let direct_key = subtitle_language_key_without_ai(&language);
    if matches!(direct_key.as_str(), "pl" | "en" | "jp" | "any") {
        return direct_key;
    }

    if let Some(base_language) = language.strip_prefix('i') {
        let base_key = subtitle_language_key_without_ai(base_language);
        if matches!(base_key.as_str(), "pl" | "en" | "jp") {
            return base_key;
        }
    }

    direct_key
}

fn subtitle_language_key_without_ai(language: &str) -> String {
    let language = language.trim().to_ascii_lowercase();

    if language == "any"
        || language == "dowolny"
        || language == "dowolne"
        || language == "wszystkie"
    {
        return "any".to_string();
    }

    if language == "pl"
        || language.contains("pol")
        || language
            .split(|character: char| !character.is_ascii_alphanumeric())
            .any(|token| token == "pl")
    {
        return "pl".to_string();
    }

    if language == "en"
        || language == "eng"
        || language.contains("ang")
        || language.contains("english")
    {
        return "en".to_string();
    }

    if language == "jp"
        || language == "ja"
        || language.contains("jap")
        || language.contains("japo")
    {
        return "jp".to_string();
    }

    language
}

fn is_ai_subtitle_language(language: &str, selected_key: &str) -> bool {
    ai_subtitle_base_key(language)
        .as_deref()
        .is_some_and(|base_key| base_key == selected_key)
}

fn ai_subtitle_base_key(language: &str) -> Option<String> {
    let normalized: String = language
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(|character| character.is_ascii_alphanumeric())
        .collect();
    let base = normalized.strip_prefix('i')?;
    let base_key = subtitle_language_key(base);

    if matches!(base_key.as_str(), "pl" | "en" | "jp") {
        Some(base_key)
    } else {
        None
    }
}

fn format_rating(raw_rating: Option<&str>) -> String {
    raw_rating
        .and_then(|rating| rating.parse::<f64>().ok())
        .map(|rating| format!("{rating:.2}").replace('.', ","))
        .unwrap_or_default()
}

fn format_episode_progress(watched: Option<&str>, total: Option<u32>) -> String {
    match (watched, total) {
        (Some(watched), Some(total)) => format!("{watched}/{total}"),
        (None, Some(total)) => format!("0/{total}"),
        (Some(watched), None) => watched.to_string(),
        (None, None) => String::new(),
    }
}

fn load_watching_availability_cache() -> WatchingAvailabilityCache {
    load_watching_availability_cache_from(&watching_availability_cache_path())
}

fn load_watching_availability_cache_from(path: &Path) -> WatchingAvailabilityCache {
    fs::read_to_string(path)
        .ok()
        .and_then(|contents| serde_json::from_str::<WatchingAvailabilityCache>(&contents).ok())
        .unwrap_or_default()
}

fn save_watching_availability_cache(cache: &WatchingAvailabilityCache) -> Result<(), String> {
    save_watching_availability_cache_to(&watching_availability_cache_path(), cache)
        .map_err(|e| command_error("watching_cache save", e))
}

fn save_watching_availability_cache_to(
    path: &Path,
    cache: &WatchingAvailabilityCache,
) -> io::Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let contents = serde_json::to_string_pretty(cache)
        .map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error))?;
    fs::write(path, contents)
}

fn watching_availability_cache_path() -> PathBuf {
    resolve_project_cache_dir().join("watching-anime-cache.json")
}

fn begin_watching_cache_refresh(
    status: &Mutex<WatchingCacheRefreshStatus>,
) -> Result<Option<WatchingCacheRefreshSummary>, String> {
    let mut status = lock_refresh_status(status)?;
    if status.running {
        return Ok(Some(WatchingCacheRefreshSummary {
            status: status.clone(),
            already_running: true,
        }));
    }

    let last_finished_at_ms = status.last_finished_at_ms;
    *status = WatchingCacheRefreshStatus {
        running: true,
        current: 0,
        total: 0,
        refreshed: 0,
        skipped: 0,
        failed: 0,
        current_title: String::new(),
        last_finished_at_ms,
        last_error: None,
    };

    Ok(None)
}

fn lock_refresh_status(
    status: &Mutex<WatchingCacheRefreshStatus>,
) -> Result<std::sync::MutexGuard<'_, WatchingCacheRefreshStatus>, String> {
    status
        .lock()
        .map_err(|_| command_error("watching_cache status", "Status lock poisoned"))
}

fn refresh_status_snapshot(
    status: &Mutex<WatchingCacheRefreshStatus>,
) -> Result<WatchingCacheRefreshStatus, String> {
    Ok(lock_refresh_status(status)?.clone())
}

fn update_refresh_status<F>(
    status: &Mutex<WatchingCacheRefreshStatus>,
    update: F,
) -> Result<(), String>
where
    F: FnOnce(&mut WatchingCacheRefreshStatus),
{
    let mut status = lock_refresh_status(status)?;
    update(&mut status);
    Ok(())
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

fn unix_timestamp_ms_u64() -> u64 {
    unix_timestamp_ms().min(u64::MAX as u128) as u64
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

fn resolve_project_cache_dir() -> PathBuf {
    if let Ok(path) = std::env::var("SHINDEN_CLIENT_CACHE_DIR") {
        if !path.trim().is_empty() {
            return PathBuf::from(path);
        }
    }

    if let Some(root) = option_env!("SHINDEN_BUILD_PROJECT_ROOT") {
        let path = PathBuf::from(root);
        if is_project_root(&path) {
            return path.join("cache");
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
            return root.join("cache");
        }
    }

    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("cache")
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
        .manage(Api(
            api,
            Mutex::new(WatchingCacheRefreshStatus::default()),
            Mutex::new(CachedUserId::default()),
        ))
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            write_log,
            test_connection,
            search,
            get_watching_anime,
            get_episodes_with_progress,
            update_anime_status,
            mark_episode_watched,
            mark_episode_unwatched,
            get_watching_cache_refresh_status,
            refresh_watching_anime_cache,
            refresh_watching_anime_cache_item,
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

    fn anime_fixture(url: &str) -> Anime {
        Anime {
            name: "Enen no Shouboutai: San no Shou".to_string(),
            url: url.to_string(),
            image_url: "https://shinden.pl/res/cover.jpg".to_string(),
            anime_type: "TV".to_string(),
            rating: "7,90".to_string(),
            episodes: "12".to_string(),
            description: String::new(),
        }
    }

    fn watching_item_fixture(
        title_id: u64,
        watch_status: Option<&str>,
        is_favourite: Option<u8>,
        episodes: Option<u32>,
    ) -> WatchingListApiItem {
        WatchingListApiItem {
            title_id,
            watch_status: watch_status.map(str::to_string),
            is_favourite,
            title: "Enen no Shouboutai: San no Shou".to_string(),
            cover_id: Some(123456),
            anime_type: Some("TV".to_string()),
            summary_rating_total: Some("7.9000".to_string()),
            episodes,
            watched_episodes_cnt: Some("3".to_string()),
            description_pl: Some("Opis".to_string()),
            description_en: None,
        }
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

    #[test]
    fn extract_user_id_from_profile_links_finds_current_user_animelist() {
        let html = r#"
            <a href="https://lista.shinden.pl/animelist/31875-szypss">Lista Anime</a>
            <a href="/user/31875-szypss">Profil</a>
        "#;

        let user_id = extract_user_id_from_profile_html(html);

        assert_eq!(user_id.as_deref(), Some("31875"));
    }

    #[test]
    fn extract_shinden_basic_auth_reads_storage_token() {
        let html = r#"<script>_Storage.basic = "token-123";</script>"#;

        assert_eq!(extract_shinden_basic_auth(html).as_deref(), Some("token-123"));
    }

    #[test]
    fn shinden_watch_status_value_maps_ui_and_api_values() {
        assert_eq!(
            shinden_watch_status_value(Some("inProgress")).unwrap(),
            Some("in progress")
        );
        assert_eq!(
            shinden_watch_status_value(Some("in progress")).unwrap(),
            Some("in progress")
        );
        assert_eq!(
            shinden_watch_status_value(Some("completed")).unwrap(),
            Some("completed")
        );
        assert_eq!(shinden_watch_status_value(Some("skip")).unwrap(), Some("skip"));
        assert_eq!(shinden_watch_status_value(Some("hold")).unwrap(), Some("hold"));
        assert_eq!(
            shinden_watch_status_value(Some("dropped")).unwrap(),
            Some("dropped")
        );
        assert_eq!(shinden_watch_status_value(Some("plan")).unwrap(), Some("plan"));
        assert_eq!(shinden_watch_status_value(Some("no")).unwrap(), None);
        assert_eq!(shinden_watch_status_value(None).unwrap(), None);
    }

    #[test]
    fn shinden_watch_status_value_rejects_unknown_status() {
        let result = shinden_watch_status_value(Some("watching-but-weird"));

        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Unsupported anime status"));
    }

    #[test]
    fn title_status_matches_normalizes_status_values() {
        assert!(title_status_matches(Some("in progress"), Some("inProgress")).unwrap());
        assert!(title_status_matches(Some("completed"), Some("completed")).unwrap());
        assert!(title_status_matches(None, Some("no")).unwrap());
        assert!(!title_status_matches(Some("completed"), Some("in progress")).unwrap());
    }

    #[test]
    fn shinden_legacy_watch_status_values_include_old_aliases() {
        assert_eq!(
            shinden_legacy_watch_status_values(Some("in progress")).unwrap(),
            vec!["in progress", "in-progress", "watching"]
        );
        assert_eq!(
            shinden_legacy_watch_status_values(Some("completed")).unwrap(),
            vec!["completed", "watched"]
        );
        assert!(shinden_legacy_watch_status_values(Some("no"))
            .unwrap()
            .is_empty());
    }

    #[test]
    fn watch_status_list_slug_maps_shinden_values() {
        assert_eq!(watch_status_list_slug("in progress"), "in-progress");
        assert_eq!(watch_status_list_slug("completed"), "completed");
        assert_eq!(watch_status_list_slug("skip"), "skip");
        assert_eq!(watch_status_list_slug("hold"), "hold");
        assert_eq!(watch_status_list_slug("dropped"), "dropped");
        assert_eq!(watch_status_list_slug("plan"), "plan");
    }

    #[test]
    fn title_status_payload_serializes_shinden_status_change() {
        let payload = build_title_status_payload(59922, Some("completed"), Some(1))
            .expect("payload should build");
        let value = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(value["input"][0]["titleId"], 59922);
        assert_eq!(value["input"][0]["watchStatus"], "completed");
        assert_eq!(value["input"][0]["isFavourite"], 1);
        assert_eq!(value["input"][0]["priority"], 0);
        assert_eq!(value["input"][0]["recommend"], 0);
    }

    #[test]
    fn title_status_payload_serializes_no_status_as_null() {
        let payload = build_title_status_payload(59922, Some("no"), None)
            .expect("payload should build");
        let value = serde_json::to_value(payload).expect("payload should serialize");

        assert!(value["input"][0]["watchStatus"].is_null());
        assert_eq!(value["input"][0]["isFavourite"], 0);
    }

    #[test]
    fn title_status_payload_preserves_priority_and_recommendation() {
        let payload =
            build_title_status_payload_with_details(59922, Some("plan"), Some(1), -10, 25)
                .expect("payload should build");
        let value = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(value["input"][0]["watchStatus"], "plan");
        assert_eq!(value["input"][0]["isFavourite"], 1);
        assert_eq!(value["input"][0]["priority"], -10);
        assert_eq!(value["input"][0]["recommend"], 25);
    }

    #[test]
    fn watched_episode_payload_serializes_single_episode() {
        let payload = build_watched_episode_payload(
            59922,
            168519,
            "2026-05-03 00:45:10".to_string(),
            1,
        );
        let value = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(value["titleId"], 59922);
        assert_eq!(value["episodes"][0]["episodeId"], 168519);
        assert_eq!(value["episodes"][0]["viewCnt"], 1);
        assert_eq!(value["episodes"][0]["createdTime"], "2026-05-03 00:45:10");
    }

    #[test]
    fn watched_episode_payload_serializes_unwatched_episode() {
        let payload = build_watched_episode_payload(
            59922,
            168519,
            "2026-05-03 00:45:10".to_string(),
            0,
        );
        let value = serde_json::to_value(payload).expect("payload should serialize");

        assert_eq!(value["titleId"], 59922);
        assert_eq!(value["episodes"][0]["episodeId"], 168519);
        assert_eq!(value["episodes"][0]["viewCnt"], 0);
        assert_eq!(value["episodes"][0]["createdTime"], "2026-05-03 00:45:10");
    }

    #[test]
    fn map_watching_list_item_builds_series_and_cover_urls() {
        let item = WatchingListApiItem {
            title_id: 59922,
            watch_status: Some("in progress".to_string()),
            is_favourite: Some(0),
            title: "Enen no Shouboutai: San no Shou".to_string(),
            cover_id: Some(123456),
            anime_type: Some("TV".to_string()),
            summary_rating_total: Some("7.9000".to_string()),
            episodes: Some(12),
            watched_episodes_cnt: Some("3".to_string()),
            description_pl: Some("Opis".to_string()),
            description_en: None,
        };

        let anime = map_watching_list_item(item).expect("item should map");

        assert_eq!(anime.name, "Enen no Shouboutai: San no Shou");
        assert_eq!(anime.url, "https://shinden.pl/series/59922");
        assert_eq!(
            anime.image_url,
            "https://cdn.shinden.eu/cdn1/images/genuine/123456.jpg"
        );
        assert_eq!(anime.anime_type, "TV");
        assert_eq!(anime.rating, "7,90");
        assert_eq!(anime.episodes, "3/12");
        assert_eq!(anime.description, "Opis");
    }

    #[test]
    fn map_watching_list_item_details_preserves_status_progress_and_favourite() {
        let item = WatchingListApiItem {
            title_id: 59922,
            watch_status: Some("in progress".to_string()),
            is_favourite: Some(1),
            title: "Enen no Shouboutai: San no Shou".to_string(),
            cover_id: Some(123456),
            anime_type: Some("TV".to_string()),
            summary_rating_total: Some("7.9000".to_string()),
            episodes: Some(12),
            watched_episodes_cnt: Some("3".to_string()),
            description_pl: Some("Opis".to_string()),
            description_en: None,
        };

        let anime = map_watching_list_item_details(item).expect("item should map");

        assert_eq!(anime.title_id, 59922);
        assert_eq!(anime.watch_status, "in progress");
        assert_eq!(anime.is_favourite, 1);
        assert_eq!(anime.name, "Enen no Shouboutai: San no Shou");
        assert_eq!(anime.url, "https://shinden.pl/series/59922");
        assert_eq!(anime.rating, "7,90");
        assert_eq!(anime.episodes, "3/12");
        assert_eq!(anime.watched_episodes_count, 3);
        assert_eq!(anime.total_episodes, Some(12));
    }

    #[test]
    fn map_search_anime_results_defaults_to_no_status_and_extracts_title_id() {
        let results = map_search_anime_results(
            vec![anime_fixture(
                "https://shinden.pl/series/59922-enen-no-shouboutai",
            )],
            Vec::new(),
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title_id, Some(59922));
        assert_eq!(results[0].watch_status, "no");
        assert_eq!(results[0].is_favourite, 0);
        assert_eq!(results[0].total_episodes, None);
        assert_eq!(results[0].anime.name, "Enen no Shouboutai: San no Shou");
    }

    #[test]
    fn map_search_anime_results_uses_matching_watching_status() {
        let results = map_search_anime_results(
            vec![anime_fixture("https://shinden.pl/titles/59922-enen-no-shouboutai")],
            vec![watching_item_fixture(
                59922,
                Some("completed"),
                Some(1),
                Some(12),
            )],
        );

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].title_id, Some(59922));
        assert_eq!(results[0].watch_status, "completed");
        assert_eq!(results[0].is_favourite, 1);
        assert_eq!(results[0].total_episodes, Some(12));
    }

    #[test]
    fn watching_list_status_url_uses_in_progress_status() {
        let url = watching_list_status_url("31875", "in progress", 100, 200);

        assert_eq!(
            url,
            "https://lista.shinden.pl/api/userlist/31875/anime/in-progress?limit=100&offset=200"
        );
    }

    #[test]
    fn watching_list_status_url_uses_selected_status_slug() {
        let url = watching_list_status_url("31875", "completed", 100, 200);

        assert_eq!(
            url,
            "https://lista.shinden.pl/api/userlist/31875/anime/completed?limit=100&offset=200"
        );
    }

    #[test]
    fn title_status_url_uses_title_and_user_ids() {
        assert_eq!(
            title_status_url(59922, "31875"),
            "https://lista.shinden.pl/api/title-status/59922/31875"
        );
    }

    #[test]
    fn legacy_userlist_series_url_uses_shinden_api_path() {
        assert_eq!(
            legacy_userlist_series_url("31875", 59922),
            "https://shinden.pl/api/userlist/31875/series/59922"
        );
    }

    #[test]
    fn title_id_from_series_url_extracts_numeric_id() {
        assert_eq!(
            title_id_from_series_url("https://shinden.pl/series/59922-enen-no-shouboutai")
                .as_deref(),
            Some("59922")
        );
        assert_eq!(
            title_id_from_series_url("https://shinden.pl/series/59922").as_deref(),
            Some("59922")
        );
        assert_eq!(
            title_id_from_series_url("https://shinden.pl/titles/59922-enen-no-shouboutai")
                .as_deref(),
            Some("59922")
        );
        assert_eq!(title_id_from_series_url("https://shinden.pl/titles/abc"), None);
    }

    #[test]
    fn true_final_episode_requires_known_total_episode_count() {
        assert!(is_true_final_episode(12, Some(12)));
        assert!(!is_true_final_episode(10, Some(12)));
        assert!(!is_true_final_episode(10, None));
    }

    #[test]
    fn true_final_episode_ignores_last_loaded_episode_when_total_is_larger() {
        let playback = vec![
            Episode {
                title: "Episode 9".to_string(),
                link: "https://shinden.pl/episode/9".to_string(),
            },
            Episode {
                title: "Episode 10".to_string(),
                link: "https://shinden.pl/episode/10".to_string(),
            },
        ];
        let progress = vec![TitleEpisodeApiItem {
            episode_id: 100,
            episode_no: 10,
            is_filer: Some(0),
            watched: None,
            title_pl: None,
            title_en: None,
            title_official: None,
        }];

        let merged = merge_episode_progress(playback, progress, Some(12));

        assert_eq!(merged[1].episode_no, 10);
        assert!(!merged[1].is_true_final_episode);
    }

    #[test]
    fn merge_episode_progress_marks_watched_rows_by_episode_number() {
        let playback = vec![
            Episode {
                title: "Playback one".to_string(),
                link: "https://shinden.pl/episode/1".to_string(),
            },
            Episode {
                title: "Playback two".to_string(),
                link: "https://shinden.pl/episode/2".to_string(),
            },
        ];
        let progress = vec![TitleEpisodeApiItem {
            episode_id: 168519,
            episode_no: 2,
            is_filer: Some(0),
            watched: Some(TitleEpisodeWatchedApiItem {
                episode_id: 168519,
                view_cnt: 1,
                created_time: Some("2022-07-28T00:33:32.000Z".to_string()),
            }),
            title_pl: Some(TitleEpisodeTitleApiItem {
                lang: "pl".to_string(),
                episode_id: 168519,
                title: "Polski tytul".to_string(),
                title_type: "national".to_string(),
            }),
            title_en: None,
            title_official: None,
        }];

        let merged = merge_episode_progress(playback, progress, Some(2));

        assert_eq!(merged[0].episode_no, 1);
        assert_eq!(merged[0].episode_id, None);
        assert!(!merged[0].watched);
        assert_eq!(merged[1].episode_id, Some(168519));
        assert_eq!(merged[1].title, "Playback two");
        assert!(merged[1].watched);
        assert_eq!(merged[1].view_count, 1);
        assert!(merged[1].is_true_final_episode);
    }

    fn watching_item(watched: Option<&str>, episodes: Option<u32>) -> WatchingListApiItem {
        WatchingListApiItem {
            title_id: 59922,
            watch_status: Some("in progress".to_string()),
            is_favourite: Some(0),
            title: "Enen no Shouboutai: San no Shou".to_string(),
            cover_id: None,
            anime_type: None,
            summary_rating_total: None,
            episodes,
            watched_episodes_cnt: watched.map(str::to_string),
            description_pl: None,
            description_en: None,
        }
    }

    fn watching_item_with_title(
        title_id: u64,
        watched: Option<&str>,
        episodes: Option<u32>,
    ) -> WatchingListApiItem {
        let mut item = watching_item(watched, episodes);
        item.title_id = title_id;
        item.title = format!("Anime {title_id}");
        item
    }

    #[test]
    fn fresh_cached_user_id_is_reused_until_ttl_expires() {
        let mut cache = CachedUserId::default();
        store_cached_user_id_value(&mut cache, "31875", 10_000);

        assert_eq!(
            cached_user_id_if_fresh(&cache, 10_000 + USER_ID_CACHE_TTL_MS - 1).as_deref(),
            Some("31875")
        );
        assert_eq!(
            cached_user_id_if_fresh(&cache, 10_000 + USER_ID_CACHE_TTL_MS + 1),
            None
        );
    }

    #[test]
    fn user_profile_rate_limit_error_is_transient() {
        assert!(is_transient_user_profile_error(
            "HTTP status client error (429 Too Many Requests) for url (https://shinden.pl/user)"
        ));
        assert!(!is_transient_user_profile_error("User is not logged in"));
    }

    #[test]
    fn watching_cache_language_scan_stops_after_first_playable_episode() {
        let mut availability = std::collections::HashMap::new();

        let should_stop = record_watching_cache_episode_subtitle_availability(
            ["EN"].into_iter(),
            Some("pl"),
            &mut availability,
        );

        assert!(should_stop);
        assert_eq!(availability.get("en"), Some(&true));
        assert_eq!(availability.get("pl"), None);
    }

    #[test]
    fn watching_cache_episode_scan_continues_past_empty_player_lists() {
        let mut availability = std::collections::HashMap::new();

        let should_stop = record_watching_cache_episode_subtitle_availability(
            std::iter::empty::<&str>(),
            Some("pl"),
            &mut availability,
        );

        assert!(!should_stop);
        assert!(availability.is_empty());
    }

    #[test]
    fn watching_cache_refresh_scans_four_titles_concurrently() {
        assert_eq!(WATCHING_CACHE_REFRESH_CONCURRENCY, 4);
    }

    #[test]
    fn watching_cache_refresh_plan_queues_only_uncached_unwatched_items() {
        let uncached = watching_item_with_title(59922, Some("2"), Some(3));
        let completed = watching_item_with_title(59923, Some("3"), Some(3));
        let cached = watching_item_with_title(59924, Some("2"), Some(3));
        let stale = watching_item_with_title(59925, Some("2"), Some(3));
        let items = vec![uncached, completed, cached, stale];
        let mut cache = WatchingAvailabilityCache::default();
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);

        cache.entries.insert(
            "59924".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59924,
                watched_episodes_cnt: 2,
                total_episodes: Some(3),
                has_available_unwatched_episode: true,
                subtitle_availability: subtitle_availability.clone(),
                checked_at_ms: 10_000,
            },
        );
        cache.entries.insert(
            "59925".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59925,
                watched_episodes_cnt: 2,
                total_episodes: Some(3),
                has_available_unwatched_episode: true,
                subtitle_availability,
                checked_at_ms: 0,
            },
        );

        let plan = collect_watching_cache_refresh_plan(&items, &cache, Some("pl"), 10_500, false);
        let queued_title_ids: Vec<u64> = plan
            .items_to_scan
            .iter()
            .map(|item| item.title_id)
            .collect();

        assert_eq!(plan.skipped, 2);
        assert_eq!(plan.processed, 2);
        assert_eq!(queued_title_ids, vec![59922, 59925]);
    }

    #[test]
    fn has_unwatched_episodes_compares_watched_count_to_total() {
        assert!(has_unwatched_episodes(&watching_item(Some("2"), Some(3))));
        assert!(!has_unwatched_episodes(&watching_item(Some("3"), Some(3))));
        assert!(has_unwatched_episodes(&watching_item(None, Some(1))));
    }

    #[test]
    fn subtitle_language_matches_common_aliases() {
        assert!(subtitle_language_matches("Polski", "PL"));
        assert!(subtitle_language_matches("Napisy PL", "polski"));
        assert!(subtitle_language_matches("iPL", "PL"));
        assert!(subtitle_language_matches("English", "EN"));
        assert!(!subtitle_language_matches("Angielski", "PL"));
    }

    #[test]
    fn subtitle_language_can_exclude_ai_translations() {
        assert!(!subtitle_language_matches_with_options("iPL", "PL", true));
        assert!(subtitle_language_matches_with_options("PL", "PL", true));
    }

    #[test]
    fn subtitle_availability_records_ai_and_human_variants_separately() {
        let mut availability = std::collections::HashMap::new();

        record_subtitle_language_availability(&mut availability, "iPL");

        assert_eq!(availability.get("pl"), Some(&true));
        assert_eq!(availability.get("pl:human"), None);

        record_subtitle_language_availability(&mut availability, "PL");

        assert_eq!(availability.get("pl"), Some(&true));
        assert_eq!(availability.get("pl:human"), Some(&true));
    }

    #[test]
    fn ai_filtered_subtitles_use_separate_cache_key() {
        let filter = WatchingAnimeFilter {
            check_subtitle_availability_online: Some(true),
            subtitle_language: Some("PL".to_string()),
            exclude_ai_subtitles: Some(true),
            ..Default::default()
        };

        assert_eq!(selected_subtitle_cache_key(&filter).as_deref(), Some("pl:human"));
    }

    #[test]
    fn watching_progress_filter_includes_all_items_when_disabled() {
        let filter = WatchingAnimeFilter::default();

        assert!(watching_progress_filter_matches(
            &watching_item(Some("3"), Some(3)),
            &filter
        ));
    }

    #[test]
    fn watching_progress_filter_uses_local_unwatched_counts() {
        let filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            ..Default::default()
        };

        assert!(watching_progress_filter_matches(
            &watching_item(Some("2"), Some(3)),
            &filter
        ));
        assert!(!watching_progress_filter_matches(
            &watching_item(Some("3"), Some(3)),
            &filter
        ));
    }

    #[test]
    fn subtitle_availability_online_check_is_opt_in() {
        assert!(!WatchingAnimeFilter::default().check_subtitle_availability_online());

        let filter = WatchingAnimeFilter {
            check_subtitle_availability_online: Some(true),
            ..Default::default()
        };

        assert!(filter.check_subtitle_availability_online());
    }

    #[test]
    fn cache_filter_hides_items_without_confirmed_available_episode() {
        let item = watching_item(Some("2"), Some(3));
        let filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            ..Default::default()
        };
        let mut cache = WatchingAvailabilityCache::default();

        assert!(!watching_cache_filter_matches(&item, &filter, &cache));

        cache.entries.insert(
            "59922".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59922,
                watched_episodes_cnt: 2,
                total_episodes: Some(3),
                has_available_unwatched_episode: false,
                subtitle_availability: Default::default(),
                checked_at_ms: 1000,
            },
        );

        assert!(!watching_cache_filter_matches(&item, &filter, &cache));
    }

    #[test]
    fn cache_filter_uses_cached_subtitle_language_availability() {
        let item = watching_item(Some("2"), Some(3));
        let filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            check_subtitle_availability_online: Some(true),
            subtitle_language: Some("PL".to_string()),
            ..Default::default()
        };
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);
        let mut cache = WatchingAvailabilityCache::default();
        cache.entries.insert(
            "59922".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59922,
                watched_episodes_cnt: 2,
                total_episodes: Some(3),
                has_available_unwatched_episode: true,
                subtitle_availability,
                checked_at_ms: 1000,
            },
        );

        assert!(watching_cache_filter_matches(&item, &filter, &cache));

        let english_filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            check_subtitle_availability_online: Some(true),
            subtitle_language: Some("EN".to_string()),
            ..Default::default()
        };

        assert!(!watching_cache_filter_matches(
            &item,
            &english_filter,
            &cache
        ));
    }

    #[test]
    fn cache_filter_rejects_entry_after_watched_count_changes() {
        let item_after_watching_episode = watching_item(Some("3"), Some(4));
        let filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            check_subtitle_availability_online: Some(true),
            subtitle_language: Some("PL".to_string()),
            ..Default::default()
        };
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);
        let mut cache = WatchingAvailabilityCache::default();
        cache.entries.insert(
            "59922".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59922,
                watched_episodes_cnt: 2,
                total_episodes: Some(4),
                has_available_unwatched_episode: true,
                subtitle_availability,
                checked_at_ms: 1000,
            },
        );

        assert!(!watching_cache_filter_matches(
            &item_after_watching_episode,
            &filter,
            &cache
        ));
    }

    #[test]
    fn cache_refresh_plan_queues_entry_after_watched_count_changes() {
        let item_after_watching_episode = watching_item(Some("3"), Some(4));
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);
        let mut cache = WatchingAvailabilityCache::default();
        cache.entries.insert(
            "59922".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59922,
                watched_episodes_cnt: 2,
                total_episodes: Some(4),
                has_available_unwatched_episode: true,
                subtitle_availability,
                checked_at_ms: 10_000,
            },
        );

        let plan = collect_watching_cache_refresh_plan(
            &[item_after_watching_episode],
            &cache,
            Some("pl"),
            10_500,
            false,
        );

        assert_eq!(plan.skipped, 0);
        assert_eq!(plan.processed, 0);
        assert_eq!(plan.items_to_scan.len(), 1);
    }

    #[test]
    fn cache_filter_distinguishes_ai_filtered_subtitle_availability() {
        let item = watching_item(Some("2"), Some(3));
        let filter = WatchingAnimeFilter {
            only_available_unwatched: Some(true),
            check_subtitle_availability_online: Some(true),
            subtitle_language: Some("PL".to_string()),
            exclude_ai_subtitles: Some(true),
            ..Default::default()
        };
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);
        let mut cache = WatchingAvailabilityCache::default();
        cache.entries.insert(
            "59922".to_string(),
            WatchingAvailabilityCacheEntry {
                title_id: 59922,
                watched_episodes_cnt: 2,
                total_episodes: Some(3),
                has_available_unwatched_episode: true,
                subtitle_availability,
                checked_at_ms: 1000,
            },
        );

        assert!(!watching_cache_filter_matches(&item, &filter, &cache));

        cache
            .entries
            .get_mut("59922")
            .expect("cache entry should exist")
            .subtitle_availability
            .insert("pl:human".to_string(), true);

        assert!(watching_cache_filter_matches(&item, &filter, &cache));
    }

    #[test]
    fn fresh_cache_entry_skips_refresh_only_when_requested_language_is_cached() {
        let item = watching_item(Some("2"), Some(3));
        let mut subtitle_availability = std::collections::HashMap::new();
        subtitle_availability.insert("pl".to_string(), true);
        let entry = WatchingAvailabilityCacheEntry {
            title_id: 59922,
            watched_episodes_cnt: 2,
            total_episodes: Some(3),
            has_available_unwatched_episode: true,
            subtitle_availability,
            checked_at_ms: 10_000,
        };

        assert!(cache_entry_satisfies_refresh(
            &entry,
            &item,
            Some("pl"),
            10_500,
            false
        ));
        assert!(!cache_entry_satisfies_refresh(
            &entry,
            &item,
            Some("en"),
            10_500,
            false
        ));
        assert!(!cache_entry_satisfies_refresh(
            &entry,
            &item,
            Some("pl"),
            10_500,
            true
        ));
    }

    #[test]
    fn watching_cache_item_error_message_hides_technical_request_details() {
        let message = watching_cache_item_error_message("Potion Wagami wo Tasukeru");

        assert_eq!(
            message,
            "Nie udalo sie sprawdzic: Potion Wagami wo Tasukeru"
        );
        assert!(!message.contains("https://"));
    }
}
