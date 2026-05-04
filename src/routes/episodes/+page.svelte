<script lang="ts">
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import type {EpisodeProgress} from "$lib/types";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {goto} from "$app/navigation";
    import Empty from "$lib/Empty.svelte";
    import { formatShindenCreatedTime, titleIdFromSeriesUrl } from "$lib/shindenProgress";
    import { queueWatchingCacheTitleRefreshFromStoredSettings } from "$lib/watchlistRefresh";

    let episodes: EpisodeProgress[] = $state([]);
    let watchedUpdateInProgress: number | null = $state(null);

    onMount(async () => {
        await loadEpisodes();
    });

    async function loadEpisodes() {
        try {
            globalStates.loadingState = LoadingState.LOADING;
            log(LogLevel.INFO, "Loading episodes");

            if (!params.titleId) {
                params.titleId = titleIdFromSeriesUrl(params.seriesUrl);
            }

            episodes = await invoke<EpisodeProgress[]>("get_episodes_with_progress", {
                url: params.seriesUrl,
                titleId: params.titleId,
                totalEpisodes: params.animeTotalEpisodes,
            });
            params.episodeProgress = episodes;
            globalStates.loadingState = LoadingState.OK;
            log(LogLevel.SUCCESS, "Loaded episodes successfully");
        } catch (e) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Error getting episodes: ${e}`);
        }
    }

    async function setEpisodeWatched(episode: EpisodeProgress, watched: boolean) {
        const titleId = params.titleId;
        if (!titleId || !episode.episodeId || episode.watched === watched) {
            return;
        }

        try {
            watchedUpdateInProgress = episode.episodeId;
            await invoke(watched ? "mark_episode_watched" : "mark_episode_unwatched", {
                titleId,
                episodeId: episode.episodeId,
                createdTime: formatShindenCreatedTime(new Date()),
            });

            episode.watched = watched;
            episode.viewCount = watched ? Math.max(episode.viewCount, 1) : 0;
            episodes = [...episodes];
            params.episodeProgress = episodes;
            queueWatchingCacheTitleRefreshFromStoredSettings(titleId);
            log(LogLevel.SUCCESS, watched
                ? `Oznaczono odcinek ${episode.episodeNo} jako obejrzany`
                : `Odznaczono odcinek ${episode.episodeNo} jako obejrzany`);
        } catch (e) {
            log(LogLevel.ERROR, `Error updating episode watched state: ${e}`);
        } finally {
            watchedUpdateInProgress = null;
        }
    }

    async function handleButton(episode: EpisodeProgress, index: number) {
        params.playersUrl = episode.link;
        params.episodeProgress = episodes;
        params.currentEpisodeIndex = index;
        await goto("/players");
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
{:else if globalStates.loadingState === LoadingState.OK}
    {#if episodes.length > 0}
    <div class="flex flex-col h-full w-full overflow-y-scroll">
        <ul class="list bg-base-100 rounded-box shadow-md">

            <li class="p-4 pb-2 text-xs opacity-60 tracking-wide">Lista odcinków:</li>

            <li class="flex items-center justify-end px-4 pb-2">
                <button class="btn btn-xs btn-ghost" onclick={() => { void loadEpisodes(); }}>
                    Odśwież
                </button>
            </li>

            {#each episodes as episode, i}
                <li class="list-row flex items-center justify-between">
                    <div class="text-4xl font-thin opacity-30 tabular-nums w-fit min-w-16 text-center">{i+1}</div>
                    <div class="list-col-grow flex-1">
                        <div>{episode.title === "" ? "Brak nazwy odcinka" : episode.title}</div>
                    </div>
                    <div class="flex shrink-0 items-center gap-2">
                        <span class={`badge ${episode.watched ? "badge-success" : "badge-ghost"}`}>
                            {episode.watched ? "Obejrzany" : "Nieobejrzany"}
                        </span>

                        <button
                            class="btn btn-sm btn-ghost"
                            disabled={!episode.episodeId || watchedUpdateInProgress === episode.episodeId}
                            onclick={() => { void setEpisodeWatched(episode, !episode.watched); }}
                        >
                            {episode.watched ? "Odznacz" : "Oznacz"}
                        </button>
                    </div>
                    <button class="btn btn-square btn-ghost" aria-label="play" onclick={async() => { await handleButton(episode, i) }}>
                        <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                    </button>
                </li>
            {/each}
        </ul>
    </div>
    {:else}
        <Empty />
    {/if}
{/if}
