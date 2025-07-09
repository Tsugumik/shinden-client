<script lang="ts">
    import {LoadingState, globalStates} from "$lib/global.svelte.js";
    import Error from "$lib/logs/Error.svelte";
    import Console from "$lib/Console.svelte";

    let htmlClass = $state("");

    let startTime: number = $state(0), endTime: number = $state(0), operationTime: number = $state(0);

    $effect(()=> {
        if (globalStates.loadingState === LoadingState.OK) {
            htmlClass = "bg-success";
        } else if (globalStates.loadingState === LoadingState.ERROR) {
            htmlClass = "bg-error";
        } else if (globalStates.loadingState === LoadingState.WARNING) {
            htmlClass = "bg-warning";
        } else {
            htmlClass = "bg-base";
        }
    });

    $effect(()=> {
        if(globalStates.loadingState === LoadingState.LOADING) {
            operationTime = 0;
            startTime = performance.now();
        } else {
            endTime = performance.now();
            operationTime = endTime - startTime;
        }
    })
</script>

<div class="flex flex-row items-center gap-2">
    <button class="btn btn-circle {htmlClass}" onclick={async()=>{globalStates.consoleState = !globalStates.consoleState}}>
        {#if globalStates.loadingState === LoadingState.LOADING}
            <span class="loading loading-spinner"></span>
            {:else if globalStates.loadingState === LoadingState.WARNING}
            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 shrink-0 stroke-current" fill="none" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M12 9v2m0 4h.01m-6.938 4h13.856c1.54 0 2.502-1.667 1.732-3L13.732 4c-.77-1.333-2.694-1.333-3.464 0L3.34 16c-.77 1.333.192 3 1.732 3z" />
            </svg>
            {:else if globalStates.loadingState === LoadingState.ERROR}
            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 shrink-0 stroke-current" fill="none" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M10 14l2-2m0 0l2-2m-2 2l-2-2m2 2l2 2m7-2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
            {:else if globalStates.loadingState === LoadingState.OK}
            <svg xmlns="http://www.w3.org/2000/svg" class="h-6 w-6 shrink-0 stroke-current" fill="none" viewBox="0 0 24 24">
                <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M9 12l2 2 4-4m6 2a9 9 0 11-18 0 9 9 0 0118 0z" />
            </svg>
        {/if}
    </button>
    <div class="font-mono">
        {#if operationTime > 0}
            {operationTime.toFixed()}ms
        {:else}
            <span class="loading loading-dots loading-md"></span>
        {/if}

    </div>
</div>

{#if globalStates.consoleState}
    <Console />
{/if}