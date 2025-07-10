<script lang="ts">
    import { onMount, onDestroy } from "svelte";
    import { invoke } from "@tauri-apps/api/core";
    import { globalStates, LoadingState, params } from "$lib/global.svelte";
    import { log, LogLevel } from "$lib/logs/logs.svelte";
    import * as dashjs from "dashjs";

    let isBuiltIn: boolean = $state(false);
    let iframeHtml: string = $state("");
    let videoElement: HTMLVideoElement | null = $state(null);
    let dashPlayer: dashjs.MediaPlayerClass | null = null;
    let pendingVideoUrl: string | null = $state(null);
    let isMp4: boolean = $state(false);

    function extractIframeSrc(iframeHtml: string): string | null {
        const parser = new DOMParser();
        const doc = parser.parseFromString(iframeHtml, "text/html");
        const iframe = doc.querySelector("iframe");
        return iframe?.getAttribute("src") ?? null;
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
            <div class="w-full h-full p-4 rounded-2xl shadow-lg flex flex-col items-center justify-center gap-2">
                <div class="w-full h-[70vh]">
                    {#if isMp4}
                    <video controls autoplay src={pendingVideoUrl}>
                    </video>
                    {:else}
                    <video
                            bind:this={videoElement}
                            class="w-full h-full"
                            autoplay
                            controls
                            crossorigin="anonymous"
                    ></video>
                    {/if}

                </div>

                <div class="text-center border bg-base-300 flex flex-col gap-2 p-4 rounded-lg w-fit items-center">
                    <h2 class="text-xl font-bold font-[Orbitron]">Built-in Player</h2>
                    <p class="text-sm">
                        powered by <a href="https://github.com/Tsugumik/cda-dl" target="_blank" class="link-accent font-mono">cda-dl</a>
                        - part of <span class="font-[Orbitron]">Shinden Client</span>
                    </p>
                    <p class="badge badge-info">{isMp4 ? "Native" : "dash.js"}</p>
                </div>
            </div>
            {:else}
            <div class="border">
                {@html iframeHtml}
            </div>
            {/if}
    {/if}
</div>
