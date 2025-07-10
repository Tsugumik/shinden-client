<script lang="ts">
    import {page} from "$app/state";
    import {invoke} from "@tauri-apps/api/core";
    import {onMount} from "svelte";
    import type {Anime} from "$lib/types";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {goto} from "$app/navigation";
    import Empty from "$lib/Empty.svelte";
    globalStates.loadingState = LoadingState.LOADING;

    let result: Array<Anime> = $state([]);

    onMount(async () => {
        try {
            log(LogLevel.INFO, `Searching anime: ${page.params.name}`);

            result = await invoke("search", {
                query: params.animeName
            });

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

    async function handleButton(url: string) {
        params.seriesUrl = url;
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

    {#if result.length > 0}

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
                    {#if anime.url.startsWith("https://shinden.pl/titles") && globalStates.user.name === null}
                        <div class="badge badge-warning">Zaloguj się aby obejrzeć</div>
                        <button disabled data-debug-url={anime.url} class="btn btn-square btn-ghost" aria-label="play" onclick={async ()=>{ await handleButton(anime.url) }}>
                            <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                        </button>
                    {:else}
                        <button data-debug-url={anime.url} class="btn btn-square btn-ghost" aria-label="play" onclick={async ()=>{ await handleButton(anime.url) }}>
                            <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                        </button>
                    {/if}

                </li>
            {/each}
        </ul>
    </div>
    {:else}
        <Empty />
    {/if}
{/if}



