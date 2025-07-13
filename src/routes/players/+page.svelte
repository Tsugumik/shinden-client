<script lang="ts">
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import type {Player} from "$lib/types";
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {goto} from "$app/navigation";
    import Empty from "$lib/Empty.svelte";
    import {builtinPlayers, dangerousPlayers, safePlayers} from "$lib/playerSafety.svelte";
    import PlayerListElement from "$lib/PlayerListElement.svelte";
    import Secure from "$lib/badges/Secure.svelte";
    import BuiltIn from "$lib/badges/BuiltIn.svelte";
    import Players from "$lib/Players.svelte";
    import Unsecure from "$lib/badges/Unsecure.svelte";
    import Unknown from "$lib/badges/Unknown.svelte";

    let players: Player[] = $state([]);
    let grouped: Record<string, Player[]> = $state({});

    let safe: string[] = $state([]);
    let unsafe: string[] = $state([]);
    let unknown: string[] = $state([]);
    let builtIn: string[] = $state([]);

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

           globalStates.loadingState = LoadingState.OK;
           log(LogLevel.SUCCESS, "Loaded players successfully");

           for (let groupedKey in grouped) {
               console.log(groupedKey);
               if(safePlayers.includes(groupedKey)) {
                   safe.push(groupedKey)
                   console.log(`SafePlayers: ${safePlayers}`);
               } else if(dangerousPlayers.includes(groupedKey)) {
                    unsafe.push(groupedKey)
               } else if(builtinPlayers.includes(groupedKey)) {
                   builtIn.push(groupedKey)
               } else {
                   unknown.push(groupedKey)
               }
           }

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
    {#if players.length > 0}
        <Players keys={builtIn} group={grouped}>
            <BuiltIn /> <Secure />
        </Players>
        <Players keys={safe} group={grouped}>
            <Secure />
        </Players>
        <Players keys={unknown} group={grouped}>
            <Unknown />
        </Players>
        <Players keys={unsafe} group={grouped}>
            <Unsecure />
        </Players>
    {:else}
    <Empty />
    {/if}
{/if}
