<script lang="ts">
    import {page} from "$app/state";
    import {invoke} from "@tauri-apps/api/core";
    import {onMount} from "svelte";
    import type {Anime} from "$lib/types";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {globalStates, LoadingState} from "$lib/global.svelte";
    globalStates.loadingState = LoadingState.LOADING;

    let result: Array<Anime> = $state([]);

    onMount(async () => {
        try {
            log(LogLevel.INFO, `Searching anime: ${page.params.name}`);

            result = await invoke("search", {
                query: page.params.name
            });

            console.log(result);

            if (result.length > 0) {
                result = result.sort((a, b) => {
                    let a_rating = Number(a.rating.replace(",", "."));
                    let b_rating = Number(b.rating.replace(",", "."));

                    return b_rating - a_rating;
                });
                globalStates.loadingState = LoadingState.OK;
                log(LogLevel.SUCCESS, `Searching anime: ${page.params.name} done`);
            } else {

                log(LogLevel.WARNING, `Searching anime: ${page.params.name} found 0 results`);
                globalStates.loadingState = LoadingState.WARNING;
            }
        } catch (e) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Error searching anime: ${page.params.name}`);
        }
    })
</script>

<div class="flex flex-col h-full w-full overflow-y-scroll">
    <ul class="list bg-base-100 rounded-box shadow-md">

        <li class="p-4 pb-2 text-xs opacity-60 tracking-wide">Wyniki wyszukiwania:</li>

        {#each result as anime}
            <li class="list-row flex items-center justify-between">
                <div class="text-4xl font-thin opacity-30 tabular-nums">{anime.rating}</div>
                <div class=""><img class="w-12 rounded-box object-fill shadow-sm" src={anime.image_url} alt="anime"/></div>
                <div class="list-col-grow flex-1">
                    <div>{anime.name}</div>
                    <div class="text-xs uppercase font-semibold opacity-60">{anime.anime_type}</div>
                </div>
                <button class="btn btn-square btn-ghost" aria-label="play">
                    <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                </button>
            </li>
        {/each}
    </ul>
</div>

