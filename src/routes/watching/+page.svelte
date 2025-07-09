<script lang="ts">
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import {page} from "$app/state";
    import {globalStates, LoadingState, params} from "$lib/global.svelte";
    import {log, LogLevel} from "$lib/logs/logs.svelte";

    let playeriframe: string = $state("");

    onMount(async () => {
        try {
            globalStates.loadingState = LoadingState.LOADING;
            log(LogLevel.INFO, "Loading player");

            playeriframe = await invoke("get_iframe", {
                id: params.playerId
            });

            globalStates.loadingState = LoadingState.OK;
            log(LogLevel.SUCCESS, "Player loaded successfully");
        } catch (e) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, "Error loading player");
        }
    });
</script>

<div class="h-full w-full flex items-center justify-center">
    {#if globalStates.loadingState === LoadingState.LOADING}
        <span class="loading loading-ring loading-xl"></span>
    {:else if globalStates.loadingState === LoadingState.OK}
        <div class="border">
            {@html playeriframe}
        </div>
    {/if}


</div>