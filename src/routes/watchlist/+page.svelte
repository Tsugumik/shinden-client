<script lang="ts">
    import { invoke } from "@tauri-apps/api/core";
    import { onMount } from "svelte";
    import { goto } from "$app/navigation";
    import type { AnimeWatchStatus, WatchingAnime } from "$lib/types";
    import { globalStates, LoadingState, params } from "$lib/global.svelte";
    import { log, LogLevel } from "$lib/logs/logs.svelte";
    import Empty from "$lib/Empty.svelte";
    import { animeStatusOptions, titleIdFromSeriesUrl } from "$lib/shindenProgress";
    import {
        watchlistSettingsStorageKey,
        type WatchingCacheRefreshStatus,
        type WatchingCacheRefreshSummary,
    } from "$lib/watchlistRefresh";

    const refreshStatusPollMs = 2000;
    const subtitleLanguageOptions = [
        { value: "PL", label: "Polski" },
        { value: "EN", label: "Angielski" },
        { value: "JP", label: "Japonski" },
        { value: "", label: "Dowolny" },
    ];

    const emptyRefreshStatus: WatchingCacheRefreshStatus = {
        running: false,
        current: 0,
        total: 0,
        refreshed: 0,
        skipped: 0,
        failed: 0,
        currentTitle: "",
        lastFinishedAtMs: null,
        lastError: null,
    };

    let result: WatchingAnime[] = $state([]);
    let showSettings = $state(false);
    let statusUpdateInProgress: number | null = $state(null);
    let onlyAvailableUnwatched = $state(false);
    let subtitleLanguage = $state("PL");
    let checkSubtitleAvailabilityOnline = $state(false);
    let excludeAiSubtitles = $state(false);
    let draftOnlyAvailableUnwatched = $state(false);
    let draftSubtitleLanguage = $state("PL");
    let draftCheckSubtitleAvailabilityOnline = $state(false);
    let draftExcludeAiSubtitles = $state(false);
    let refreshStatus: WatchingCacheRefreshStatus = $state({ ...emptyRefreshStatus });
    let lastSeenRefreshed = $state(0);
    let lastSeenRefreshFinishedAtMs: number | null = $state(null);
    let refreshStatusInitialized = $state(false);
    let silentReloadInProgress = $state(false);

    onMount(() => {
        loadSettings();
        void loadWatchingAnime();
        void pollRefreshStatus();

        const statusTimer = window.setInterval(() => {
            void pollRefreshStatus();
        }, refreshStatusPollMs);

        return () => {
            window.clearInterval(statusTimer);
        };
    });

    function loadSettings() {
        const storedSettings = localStorage.getItem(watchlistSettingsStorageKey);
        if (!storedSettings) {
            return;
        }

        try {
            const parsedSettings = JSON.parse(storedSettings);
            onlyAvailableUnwatched = Boolean(parsedSettings.onlyAvailableUnwatched);
            subtitleLanguage =
                typeof parsedSettings.subtitleLanguage === "string"
                    ? parsedSettings.subtitleLanguage
                    : "PL";
            checkSubtitleAvailabilityOnline = Boolean(
                parsedSettings.checkSubtitleAvailabilityOnline,
            );
            excludeAiSubtitles = Boolean(parsedSettings.excludeAiSubtitles);
        } catch (e) {
            log(LogLevel.WARNING, `Error loading watchlist settings: ${e}`);
        }
    }

    function saveSettings() {
        localStorage.setItem(
            watchlistSettingsStorageKey,
            JSON.stringify({
                onlyAvailableUnwatched,
                subtitleLanguage,
                checkSubtitleAvailabilityOnline,
                excludeAiSubtitles,
            }),
        );
    }

    function currentFilter() {
        return {
            onlyAvailableUnwatched,
            subtitleLanguage,
            checkSubtitleAvailabilityOnline,
            excludeAiSubtitles,
        };
    }

    async function pollRefreshStatus() {
        try {
            const nextStatus = await invoke<WatchingCacheRefreshStatus>(
                "get_watching_cache_refresh_status",
            );

            refreshStatus = nextStatus;
            if (!refreshStatusInitialized) {
                refreshStatusInitialized = true;
                lastSeenRefreshed = nextStatus.refreshed;
                lastSeenRefreshFinishedAtMs = nextStatus.lastFinishedAtMs;
                return;
            }

            const finishedRefreshHasNewData =
                nextStatus.lastFinishedAtMs !== null &&
                nextStatus.lastFinishedAtMs !== lastSeenRefreshFinishedAtMs;
            const cacheHasNewData =
                nextStatus.refreshed > lastSeenRefreshed || finishedRefreshHasNewData;
            if (cacheHasNewData && !silentReloadInProgress) {
                lastSeenRefreshed = nextStatus.refreshed;
                lastSeenRefreshFinishedAtMs = nextStatus.lastFinishedAtMs;
                await loadWatchingAnime(false);
            } else if (!nextStatus.running) {
                lastSeenRefreshed = nextStatus.refreshed;
                lastSeenRefreshFinishedAtMs = nextStatus.lastFinishedAtMs;
            }
        } catch (e) {
            log(LogLevel.ERROR, `Error loading watchlist refresh status: ${e}`);
        }
    }

    async function refreshWatchingCache(force: boolean) {
        if (refreshStatus.running) {
            return;
        }

        try {
            refreshStatus = {
                ...refreshStatus,
                running: true,
                refreshed: 0,
                skipped: 0,
                failed: 0,
                currentTitle: "",
                lastError: null,
            };
            lastSeenRefreshed = 0;

            const summary = await invoke<WatchingCacheRefreshSummary>(
                "refresh_watching_anime_cache",
                {
                    filter: currentFilter(),
                    force,
                },
            );
            refreshStatus = summary.status;
            lastSeenRefreshed = summary.status.refreshed;
            lastSeenRefreshFinishedAtMs = summary.status.lastFinishedAtMs;

            if (!summary.alreadyRunning) {
                await loadWatchingAnime(false);
            }
        } catch (e) {
            refreshStatus = {
                ...refreshStatus,
                running: false,
                lastError: `${e}`,
            };
            log(LogLevel.ERROR, `Error refreshing watched anime cache: ${e}`);
            await pollRefreshStatus();
        }
    }

    function formatRefreshTime(timestamp: number | null) {
        if (!timestamp) {
            return "";
        }

        return new Date(timestamp).toLocaleTimeString();
    }

    function formatRefreshError(error: string) {
        if (error.includes("error sending request")) {
            return "Nie udalo sie sprawdzic czesci odcinkow. Sprobuj odswiezyc dane ponownie.";
        }

        return error;
    }

    async function loadWatchingAnime(showLoading = true) {
        if (!showLoading && silentReloadInProgress) {
            return;
        }

        try {
            if (showLoading) {
                globalStates.loadingState = LoadingState.LOADING;
                log(LogLevel.INFO, "Loading watched anime list");
            } else {
                silentReloadInProgress = true;
            }

            result = await invoke<WatchingAnime[]>("get_watching_anime", {
                filter: {
                    onlyAvailableUnwatched,
                    subtitleLanguage,
                    checkSubtitleAvailabilityOnline,
                    excludeAiSubtitles,
                },
            });

            if (showLoading || globalStates.loadingState !== LoadingState.LOADING) {
                globalStates.loadingState =
                    result.length > 0 ? LoadingState.OK : LoadingState.WARNING;
            }
            if (showLoading) {
                log(LogLevel.SUCCESS, "Loaded watched anime list");
            }
        } catch (e) {
            if (showLoading) {
                globalStates.loadingState = LoadingState.ERROR;
            }
            log(LogLevel.ERROR, `Error loading watched anime list: ${e}`);
        } finally {
            if (!showLoading) {
                silentReloadInProgress = false;
            }
        }
    }

    function openSettings() {
        draftOnlyAvailableUnwatched = onlyAvailableUnwatched;
        draftSubtitleLanguage = subtitleLanguage;
        draftCheckSubtitleAvailabilityOnline = checkSubtitleAvailabilityOnline;
        draftExcludeAiSubtitles = excludeAiSubtitles;
        showSettings = true;
    }

    function closeSettings() {
        showSettings = false;
    }

    async function applySettings() {
        onlyAvailableUnwatched = draftOnlyAvailableUnwatched;
        subtitleLanguage = draftSubtitleLanguage;
        checkSubtitleAvailabilityOnline = draftCheckSubtitleAvailabilityOnline;
        excludeAiSubtitles = draftExcludeAiSubtitles;
        saveSettings();
        showSettings = false;
        await loadWatchingAnime();
    }

    async function updateStatus(anime: WatchingAnime, status: AnimeWatchStatus) {
        if (anime.watchStatus === status) {
            return;
        }

        try {
            statusUpdateInProgress = anime.titleId;
            await invoke("update_anime_status", {
                titleId: anime.titleId,
                status,
                isFavourite: anime.isFavourite,
            });

            anime.watchStatus = status;
            result = status === "in progress"
                ? [...result]
                : result.filter((item) => item.titleId !== anime.titleId);
            log(LogLevel.SUCCESS, `Zmieniono status anime: ${anime.name}`);
        } catch (e) {
            log(LogLevel.ERROR, `Error updating anime status: ${e}`);
        } finally {
            statusUpdateInProgress = null;
        }
    }

    async function handleStatusChange(anime: WatchingAnime, event: Event) {
        const status = (event.currentTarget as HTMLSelectElement).value as AnimeWatchStatus;
        await updateStatus(anime, status);
    }

    async function handleButton(anime: WatchingAnime) {
        params.seriesUrl = anime.url;
        params.titleId = anime.titleId || titleIdFromSeriesUrl(anime.url);
        params.animeWatchStatus = anime.watchStatus;
        params.animeIsFavourite = anime.isFavourite;
        params.animeTotalEpisodes = anime.totalEpisodes;
        params.episodeProgress = [];
        params.currentEpisodeIndex = -1;
        await goto("/episodes");
    }
</script>

{#if globalStates.loadingState === LoadingState.LOADING}
    <div class="flex w-full h-full flex-col gap-4 p-4">
        <div class="skeleton h-32 w-full"></div>
        <div class="skeleton h-32 w-full"></div>
        <div class="skeleton h-32 w-full"></div>
        <div class="skeleton h-32 w-full"></div>
        <div class="skeleton h-32 w-full"></div>
    </div>
{:else}
    <div class="flex flex-col h-full w-full overflow-y-scroll gap-3 p-4">
        <div class="flex items-center justify-between gap-3 bg-base-100 rounded-box shadow-md p-4">
            <div class="min-w-0">
                <div class="text-xs opacity-60 tracking-wide uppercase">
                    {onlyAvailableUnwatched ? "Do nadrobienia" : "Lista ogladanych anime"}
                </div>
                <div class="text-sm opacity-80 truncate">
                    {result.length} pozycji
                    {#if onlyAvailableUnwatched && checkSubtitleAvailabilityOnline}
                        | napisy: {subtitleLanguage || "dowolny"}
                        {#if excludeAiSubtitles}
                            bez AI
                        {/if}
                    {:else if onlyAvailableUnwatched}
                        | dostepne z cache
                    {/if}
                </div>
                {#if refreshStatus.running}
                    <div class="text-xs text-info truncate">
                        Odswiezanie {refreshStatus.current}/{refreshStatus.total}
                        {#if refreshStatus.currentTitle}
                            | {refreshStatus.currentTitle}
                        {/if}
                        {#if refreshStatus.failed}
                            | bledy: {refreshStatus.failed}
                        {/if}
                    </div>
                {:else if refreshStatus.lastFinishedAtMs}
                    <div class="text-xs opacity-60 truncate">
                        Cache {formatRefreshTime(refreshStatus.lastFinishedAtMs)}
                        | odswiezone: {refreshStatus.refreshed}
                        | bez zmian: {refreshStatus.skipped}
                        {#if refreshStatus.failed}
                            | bledy: {refreshStatus.failed}
                        {/if}
                    </div>
                {/if}
                {#if refreshStatus.lastError}
                    <div class="text-xs text-warning truncate">
                        {formatRefreshError(refreshStatus.lastError)}
                    </div>
                {/if}
            </div>

            <div class="flex shrink-0 items-center gap-1">
                <button
                    class="btn btn-square btn-ghost btn-sm"
                    aria-label="odswiez dane listy"
                    title="Odswiez dane listy"
                    disabled={refreshStatus.running}
                    onclick={() => { void refreshWatchingCache(true); }}
                >
                    <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                        <g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor">
                            <path d="M21 12a9 9 0 0 1-15.5 6.3"></path>
                            <path d="M3 12a9 9 0 0 1 15.5-6.3"></path>
                            <path d="M3 19v-5h5"></path>
                            <path d="M21 5v5h-5"></path>
                        </g>
                    </svg>
                </button>

                <button
                    class="btn btn-square btn-ghost btn-sm"
                    aria-label="ustawienia listy"
                    title="Ustawienia listy"
                    onclick={openSettings}
                >
                    <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                        <g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor">
                            <path d="M12 15.5A3.5 3.5 0 1 0 12 8a3.5 3.5 0 0 0 0 7.5z"></path>
                            <path d="M19.4 15a1.7 1.7 0 0 0 .3 1.9l.1.1a2 2 0 1 1-2.8 2.8l-.1-.1a1.7 1.7 0 0 0-1.9-.3 1.7 1.7 0 0 0-1 1.6V21a2 2 0 1 1-4 0v-.1a1.7 1.7 0 0 0-1-1.6 1.7 1.7 0 0 0-1.9.3l-.1.1a2 2 0 1 1-2.8-2.8l.1-.1A1.7 1.7 0 0 0 4.6 15 1.7 1.7 0 0 0 3 14H3a2 2 0 1 1 0-4h.1a1.7 1.7 0 0 0 1.6-1 1.7 1.7 0 0 0-.3-1.9l-.1-.1a2 2 0 1 1 2.8-2.8l.1.1a1.7 1.7 0 0 0 1.9.3h.1A1.7 1.7 0 0 0 10 3V3a2 2 0 1 1 4 0v.1a1.7 1.7 0 0 0 1 1.6h.1a1.7 1.7 0 0 0 1.9-.3l.1-.1A2 2 0 1 1 20 7.1l-.1.1a1.7 1.7 0 0 0-.3 1.9v.1A1.7 1.7 0 0 0 21 10h.1a2 2 0 1 1 0 4H21a1.7 1.7 0 0 0-1.6 1z"></path>
                        </g>
                    </svg>
                </button>
            </div>
        </div>

        {#if result.length > 0}
        <ul class="list bg-base-100 rounded-box shadow-md">
            {#each result as anime}
                <li class="list-row flex items-center justify-between">
                    <div class="text-4xl font-thin opacity-30 tabular-nums">
                        {anime.rating || "-"}
                    </div>
                    <div>
                        <img
                            class="w-12 rounded-box object-fill shadow-sm"
                            src={anime.image_url}
                            alt="anime"
                        />
                    </div>
                    <div class="list-col-grow flex-1">
                        <div>{anime.name}</div>
                        <div class="text-xs uppercase font-semibold opacity-60">
                            {anime.anime_type}
                            {#if anime.episodes}
                                <span class="normal-case"> | {anime.episodes}</span>
                            {/if}
                        </div>
                    </div>
                    <select
                        class="select select-bordered select-sm w-36"
                        value={anime.watchStatus}
                        disabled={statusUpdateInProgress === anime.titleId}
                        aria-label="status anime"
                        onchange={(event) => { void handleStatusChange(anime, event); }}
                    >
                        {#each animeStatusOptions as option}
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                    <button
                        data-debug-url={anime.url}
                        class="btn btn-square btn-ghost"
                        aria-label="episodes"
                        onclick={async () => { await handleButton(anime); }}
                    >
                        <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                            <g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor">
                                <path d="M6 3L20 12 6 21 6 3z"></path>
                            </g>
                        </svg>
                    </button>
                </li>
            {/each}
        </ul>
        {:else}
            <Empty />
        {/if}
    </div>
{/if}

{#if showSettings}
    <div class="fixed inset-0 z-50 flex items-center justify-center bg-black/50 p-4">
        <section class="w-full max-w-md rounded-box bg-base-100 p-4 shadow-xl border border-base-300">
            <div class="flex items-center justify-between gap-3">
                <h2 class="text-base font-semibold">Ustawienia listy</h2>
                <button class="btn btn-square btn-ghost btn-sm" aria-label="zamknij" onclick={closeSettings}>
                    &#x2715;
                </button>
            </div>

            <div class="mt-4 flex flex-col gap-4">
                <label class="flex items-center justify-between gap-4">
                    <span class="text-sm">Tylko z nieobejrzanym odcinkiem</span>
                    <input
                        type="checkbox"
                        class="toggle toggle-primary"
                        bind:checked={draftOnlyAvailableUnwatched}
                    />
                </label>

                <label class="flex items-center justify-between gap-4">
                    <span class="text-sm">Filtruj po jezyku napisow</span>
                    <input
                        type="checkbox"
                        class="toggle toggle-primary"
                        bind:checked={draftCheckSubtitleAvailabilityOnline}
                        disabled={!draftOnlyAvailableUnwatched}
                    />
                </label>

                {#if draftOnlyAvailableUnwatched && draftCheckSubtitleAvailabilityOnline}
                    <p class="text-xs opacity-60">
                        Filtrowanie po jezyku wydluza odswiezanie, bo aplikacja sprawdza playery dla nieobejrzanych odcinkow.
                    </p>
                {/if}

                <label class="form-control w-full">
                    <span class="label-text mb-2">Jezyk napisow</span>
                    <select
                        class="select select-bordered w-full"
                        bind:value={draftSubtitleLanguage}
                        disabled={!draftOnlyAvailableUnwatched || !draftCheckSubtitleAvailabilityOnline}
                    >
                        {#each subtitleLanguageOptions as option}
                            <option value={option.value}>{option.label}</option>
                        {/each}
                    </select>
                </label>

                <label class="flex items-center justify-between gap-4">
                    <span class="text-sm">Wyklucz napisy AI (iPL)</span>
                    <input
                        type="checkbox"
                        class="toggle toggle-primary"
                        bind:checked={draftExcludeAiSubtitles}
                        disabled={!draftOnlyAvailableUnwatched || !draftCheckSubtitleAvailabilityOnline}
                    />
                </label>
            </div>

            <div class="mt-5 flex justify-end gap-2">
                <button class="btn btn-ghost" onclick={closeSettings}>Anuluj</button>
                <button class="btn btn-primary" onclick={applySettings}>Zastosuj</button>
            </div>
        </section>
    </div>
{/if}
