<script lang="ts">
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import type {Episode} from "$lib/types";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {goto} from "$app/navigation";

    let episodes: Episode[] = $state([]);

    onMount(async ()=>{
       try {
           globalStates.loadingState = LoadingState.LOADING;
           log(LogLevel.INFO, "Loading episodes");
           episodes = await invoke("get_episodes", {
               url: params.seriesUrl
           });
           globalStates.loadingState = LoadingState.OK;
           log(LogLevel.SUCCESS, "Loaded episodes successfully");
       } catch (e) {
           globalStates.loadingState = LoadingState.ERROR;
           log(LogLevel.ERROR, `Error getting episodes: ${e}`);
       }
    });

    async function handleButton(url: string) {
        params.playersUrl = url;
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
    <div class="flex flex-col h-full w-full overflow-y-scroll">
        <ul class="list bg-base-100 rounded-box shadow-md">

            <li class="p-4 pb-2 text-xs opacity-60 tracking-wide">Lista odcinków:</li>

            {#each episodes as episode, i}
                <li class="list-row flex items-center justify-between">
                    <div class="text-4xl font-thin opacity-30 tabular-nums w-16 text-center">{i+1}</div>
<!--                    <div class=""><img class="w-12 rounded-box object-fill shadow-sm" src={anime.image_url} alt="anime"/></div>-->
                    <div class="list-col-grow flex-1">
                        <div>{episode.title}</div>
                    </div>
                    <button class="btn btn-square btn-ghost" aria-label="play" onclick={async() => { await handleButton(episode.link) }}>
                        <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                    </button>
                </li>
            {/each}
        </ul>
    </div>
{/if}