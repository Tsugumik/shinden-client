import { invoke } from "@tauri-apps/api/core";

export const watchlistSettingsStorageKey = "shinden-watchlist-settings";
export const watchlistAutoRefreshMs = 15 * 60 * 1000;
export const watchlistPostProgressRefreshDelayMs = 1500;

export type WatchlistRefreshFilter = {
    onlyAvailableUnwatched: boolean;
    subtitleLanguage: string;
    checkSubtitleAvailabilityOnline: boolean;
    excludeAiSubtitles: boolean;
};

export type WatchingCacheRefreshStatus = {
    running: boolean;
    current: number;
    total: number;
    refreshed: number;
    skipped: number;
    failed: number;
    currentTitle: string;
    lastFinishedAtMs: number | null;
    lastError: string | null;
};

export type WatchingCacheRefreshSummary = {
    status: WatchingCacheRefreshStatus;
    alreadyRunning: boolean;
};

const defaultWatchlistRefreshFilter: WatchlistRefreshFilter = {
    onlyAvailableUnwatched: false,
    subtitleLanguage: "PL",
    checkSubtitleAvailabilityOnline: false,
    excludeAiSubtitles: false,
};

let stopBackgroundRefresh: (() => void) | null = null;
const queuedTitleRefreshTimers = new Map<number, ReturnType<typeof window.setTimeout>>();

export function loadWatchlistRefreshFilter(): WatchlistRefreshFilter {
    if (typeof localStorage === "undefined") {
        return { ...defaultWatchlistRefreshFilter };
    }

    const storedSettings = localStorage.getItem(watchlistSettingsStorageKey);
    if (!storedSettings) {
        return { ...defaultWatchlistRefreshFilter };
    }

    try {
        const parsedSettings = JSON.parse(storedSettings);
        return {
            onlyAvailableUnwatched: Boolean(parsedSettings.onlyAvailableUnwatched),
            subtitleLanguage:
                typeof parsedSettings.subtitleLanguage === "string"
                    ? parsedSettings.subtitleLanguage
                    : "PL",
            checkSubtitleAvailabilityOnline: Boolean(
                parsedSettings.checkSubtitleAvailabilityOnline,
            ),
            excludeAiSubtitles: Boolean(parsedSettings.excludeAiSubtitles),
        };
    } catch {
        return { ...defaultWatchlistRefreshFilter };
    }
}

export async function refreshWatchingCacheFromStoredSettings(force = false) {
    return invoke<WatchingCacheRefreshSummary>("refresh_watching_anime_cache", {
        filter: loadWatchlistRefreshFilter(),
        force,
    });
}

export async function refreshWatchingCacheTitleFromStoredSettings(
    titleId: number,
    force = true,
) {
    return invoke<WatchingCacheRefreshSummary>("refresh_watching_anime_cache_item", {
        titleId,
        filter: loadWatchlistRefreshFilter(),
        force,
    });
}

export function queueWatchingCacheTitleRefreshFromStoredSettings(
    titleId: number,
    delayMs = watchlistPostProgressRefreshDelayMs,
) {
    if (typeof window === "undefined") {
        void refreshWatchingCacheTitleFromStoredSettings(titleId).catch(() => {});
        return;
    }

    const queuedTimer = queuedTitleRefreshTimers.get(titleId);
    if (queuedTimer) {
        window.clearTimeout(queuedTimer);
    }

    const timer = window.setTimeout(() => {
        queuedTitleRefreshTimers.delete(titleId);
        void refreshWatchingCacheTitleFromStoredSettings(titleId).catch(() => {});
    }, delayMs);

    queuedTitleRefreshTimers.set(titleId, timer);
}

export function startWatchlistBackgroundRefresh() {
    if (typeof window === "undefined") {
        return () => {};
    }

    if (stopBackgroundRefresh) {
        return stopBackgroundRefresh;
    }

    const timer = window.setInterval(() => {
        void refreshWatchingCacheFromStoredSettings(false).catch(() => {});
    }, watchlistAutoRefreshMs);

    stopBackgroundRefresh = () => {
        window.clearInterval(timer);
        stopBackgroundRefresh = null;
    };

    return stopBackgroundRefresh;
}
