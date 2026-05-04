<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { globalStates, LoadingState, params } from "$lib/global.svelte";
    import { log, LogLevel } from "$lib/logs/logs.svelte";
    import * as dashjs from "dashjs";
    import { goto } from "$app/navigation";
    import { formatShindenCreatedTime } from "$lib/shindenProgress";
    import { queueWatchingCacheTitleRefreshFromStoredSettings } from "$lib/watchlistRefresh";
    import type { EpisodeProgress } from "$lib/types";

    let isBuiltIn: boolean = $state(false);
    let iframeHtml: string = $state("");
    let videoElement: HTMLVideoElement | null = $state(null);
    let dashPlayer: dashjs.MediaPlayerClass | null = null;
    let pendingVideoUrl: string | null = $state(null);
    let isMp4: boolean = $state(false);
    let progressWriteInProgress = $state(false);

    function extractIframeSrc(iframeHtml: string): string | null {
        const parser = new DOMParser();
        const doc = parser.parseFromString(iframeHtml, "text/html");
        const iframe = doc.querySelector("iframe");
        return iframe?.getAttribute("src") ?? null;
    }

    function currentEpisode(): EpisodeProgress | null {
        return params.episodeProgress[params.currentEpisodeIndex] ?? null;
    }

    function previousEpisode(): EpisodeProgress | null {
        return params.currentEpisodeIndex > 0
            ? params.episodeProgress[params.currentEpisodeIndex - 1]
            : null;
    }

    function nextEpisode(): EpisodeProgress | null {
        return params.currentEpisodeIndex >= 0 && params.currentEpisodeIndex < params.episodeProgress.length - 1
            ? params.episodeProgress[params.currentEpisodeIndex + 1]
            : null;
    }

    function primaryProgressLabel() {
        const episode = currentEpisode();
        if (!episode) {
            return "Oznacz jako obejrzane";
        }

        return episode.isTrueFinalEpisode || !nextEpisode()
            ? "Oznacz jako obejrzane"
            : "Oznacz jako obejrzane i przejdź dalej";
    }

    async function goToEpisode(episode: EpisodeProgress) {
        const index = params.episodeProgress.findIndex((item) => item.link === episode.link);
        if (index < 0) {
            return;
        }

        params.currentEpisodeIndex = index;
        params.playersUrl = episode.link;
        params.playerId = "";
        await goto("/players");
    }

    async function markCurrentEpisodeWatched() {
        const episode = currentEpisode();
        const titleId = params.titleId;
        if (!episode || !titleId || !episode.episodeId || episode.watched) {
            return;
        }

        await invoke("mark_episode_watched", {
            titleId,
            episodeId: episode.episodeId,
            createdTime: formatShindenCreatedTime(new Date()),
        });

        episode.watched = true;
        episode.viewCount = Math.max(episode.viewCount, 1);
        params.episodeProgress = [...params.episodeProgress];
        queueWatchingCacheTitleRefreshFromStoredSettings(titleId);
    }

    async function handlePrimaryProgressAction() {
        const episode = currentEpisode();
        if (!episode) {
            return;
        }

        try {
            progressWriteInProgress = true;
            await markCurrentEpisodeWatched();

            if (episode.isTrueFinalEpisode && params.titleId) {
                await invoke("update_anime_status", {
                    titleId: params.titleId,
                    status: "completed",
                    isFavourite: params.animeIsFavourite,
                });
                params.animeWatchStatus = "completed";
                log(LogLevel.SUCCESS, "Oznaczono anime jako obejrzane");
                return;
            }

            const next = nextEpisode();
            if (next) {
                await goToEpisode(next);
            }
        } catch (e) {
            log(LogLevel.ERROR, `Error updating current episode progress: ${e}`);
        } finally {
            progressWriteInProgress = false;
        }
    }


    onMount(async () => {
        try {
            globalStates.loadingState = LoadingState.LOADING;
            log(LogLevel.INFO, "Loading player...");

            const rawIframe = await invoke<string>("get_iframe", {
                id: params.playerId
            });

            const iframeSrc = extractIframeSrc(rawIframe);
            if (!iframeSrc) throw new Error("Iframe src not found");

            if (iframeSrc.includes("cda.pl")) {
                log(LogLevel.INFO, `Detected CDA source`);

                pendingVideoUrl = await invoke<string>("get_cda_video", {
                    url: iframeSrc
                });

                log(LogLevel.SUCCESS, `Resolved CDA video URL`);



                if(pendingVideoUrl.endsWith("mpd")) {
                    isMp4 = false;
                } else {
                    isMp4 = true;
                }

                isBuiltIn = true;
            } else {
                iframeHtml = rawIframe;
                isBuiltIn = false;
                log(LogLevel.INFO, "Using raw iframe");
            }

            globalStates.loadingState = LoadingState.OK;
        } catch (e) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Error loading player: ${e}`);
        }
    })


    $effect(()=>{
        if (isBuiltIn && pendingVideoUrl && videoElement && !isMp4) {
            dashPlayer?.reset();
            dashPlayer = dashjs.MediaPlayer().create();
            dashPlayer.initialize(videoElement, pendingVideoUrl, true);
            dashPlayer.setAutoPlay(true);
            //pendingVideoUrl = null;
        }
    })

    onDestroy(() => {
        dashPlayer?.reset();
        dashPlayer = null;
    });
</script>

<div class="h-full w-full flex items-center justify-center">
    {#if globalStates.loadingState === LoadingState.LOADING}
        <span class="loading loading-ring loading-xl"></span>
    {:else if globalStates.loadingState === LoadingState.OK}

            {#if isBuiltIn}
            <div class="w-full h-full p-4 pb-28 md:p-6 md:pb-28 flex flex-col items-center justify-center gap-4">
                <div class="w-full flex-1 flex items-center justify-center min-h-0">
                    {#if isMp4}
                    <video class="block max-w-full max-h-full rounded-2xl shadow-2xl outline-none" controls autoplay src={pendingVideoUrl}>
                    </video>
                    {:else}
                    <video
                            bind:this={videoElement}
                            class="block max-w-full max-h-full rounded-2xl shadow-2xl outline-none"
                            autoplay
                            controls
                            crossorigin="anonymous"
                    ></video>
                    {/if}
                </div>

                <div class="w-full shrink-0 flex flex-col sm:flex-row items-center justify-between bg-base-300 shadow-md border border-base-content/5 rounded-xl px-5 py-2 gap-3">
                    <div class="flex items-center gap-3">
                        <h2 class="text-lg font-bold font-[Orbitron]">Built-in Player</h2>
                        <p class="badge badge-info badge-sm shadow-sm">{isMp4 ? "Native" : "dash.js"}</p>
                    </div>
                    <p class="text-xs text-base-content/70 text-center sm:text-right">
                        powered by <a href="https://github.com/Tsugumik/cda-dl" target="_blank" class="link link-hover link-accent font-mono">cda-dl</a>
                        - part of <span class="font-[Orbitron]">Shinden Client</span>
                    </p>
                </div>
            </div>
            {:else}
            <div class="w-full h-full p-4 pb-28 md:p-6 md:pb-28 flex items-center justify-center">
                <div class="w-full max-w-7xl max-h-full rounded-2xl shadow-2xl overflow-hidden [&>iframe]:block [&>iframe]:w-full [&>iframe]:aspect-video [&>iframe]:max-h-full">
                    {@html iframeHtml}
                </div>
            </div>
            {/if}

            {#if currentEpisode()}
                <div class="fixed bottom-4 left-4 right-4 z-20 flex flex-col sm:flex-row items-center justify-center gap-2 rounded-box bg-base-300/95 border border-base-content/10 p-3 shadow-xl">
                    <button
                        class="btn btn-primary btn-sm w-full sm:w-auto"
                        disabled={progressWriteInProgress || !currentEpisode()?.episodeId}
                        onclick={() => { void handlePrimaryProgressAction(); }}
                    >
                        {primaryProgressLabel()}
                    </button>

                    {#if nextEpisode() && !currentEpisode()?.isTrueFinalEpisode}
                        <button
                            class="btn btn-ghost btn-sm w-full sm:w-auto"
                            disabled={progressWriteInProgress}
                            onclick={() => {
                                const episode = nextEpisode();
                                if (episode) void goToEpisode(episode);
                            }}
                        >
                            Przejdź dalej
                        </button>
                    {/if}

                    {#if previousEpisode()}
                        <button
                            class="btn btn-ghost btn-sm w-full sm:w-auto"
                            disabled={progressWriteInProgress}
                            onclick={() => {
                                const episode = previousEpisode();
                                if (episode) void goToEpisode(episode);
                            }}
                        >
                            Do poprzedniego odcinka
                        </button>
                    {/if}
                </div>
            {/if}
    {/if}
</div>
