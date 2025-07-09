<script lang="ts">
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import type {Player} from "$lib/types";
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {goto} from "$app/navigation";

    let players: Player[] = $state([]);
    let grouped: Record<string, Player[]> = $state({});

    onMount(async ()=>{
       try {
           globalStates.loadingState = LoadingState.LOADING;
           log(LogLevel.INFO, "Loading players");

           if(!params.playersUrl) {
               await goto("/");
               log(LogLevel.WARNING, "No parameters provided; probably refreshing page");
               return;
           }

           players = await invoke("get_players", {
               url: params.playersUrl
           });

           grouped = players.reduce<Record<string, Player[]>>((acc, p) => {
               if (!acc[p.player]) {
                   acc[p.player] = [];
               }
               acc[p.player].push(p);
               return acc;
           }, {});

           console.log(grouped);

           globalStates.loadingState = LoadingState.OK;
           log(LogLevel.SUCCESS, "Loaded players successfully");

       } catch (e) {
           globalStates.loadingState = LoadingState.ERROR;
           log(LogLevel.ERROR, `Error loading players: ${e}`);
       }
    });
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
    {#each Object.keys(grouped) as playerName}

        <div class="collapse collapse-arrow bg-base-100">
            <input type="checkbox" />
            <div class="collapse-title font-semibold">{playerName}</div>
            <div class="collapse-content text-sm">
                <ul class="list">
                    {#each grouped[playerName] as player, i}
                        <li class="list-row flex items-center justify-between">
                            <div class="text-4xl font-thin opacity-30 tabular-nums w-16 text-center">{i+1}</div>
                            <div class="list-col-grow flex-1">
                                <div>Język napisów: {player.lang_subs}</div>
                                <div>Język audio: {player.lang_audio}</div>
                            </div>
                            <div class="text-4xl font-thin opacity-30 tabular-nums w-24 text-center">{player.max_res}</div>
                            <a class="btn btn-square btn-ghost" aria-label="play" href="/watching/{player.online_id}">
                                <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
                            </a>
                        </li>
                    {/each}
                </ul>
            </div>
        </div>
    {/each}
{/if}
