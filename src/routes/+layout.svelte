<script lang="ts">
    import "../app.css";
    import Navbar from "$lib/Navbar.svelte";
    import { onDestroy } from "svelte";
    import { globalStates } from "$lib/global.svelte";
    import { startWatchlistBackgroundRefresh } from "$lib/watchlistRefresh";

    let { children } = $props();
    let stopWatchlistBackgroundRefresh: (() => void) | null = null;

    $effect(() => {
        if (globalStates.user.name && !stopWatchlistBackgroundRefresh) {
            stopWatchlistBackgroundRefresh = startWatchlistBackgroundRefresh();
        } else if (!globalStates.user.name && stopWatchlistBackgroundRefresh) {
            stopWatchlistBackgroundRefresh();
            stopWatchlistBackgroundRefresh = null;
        }
    });

    onDestroy(() => {
        stopWatchlistBackgroundRefresh?.();
    });
</script>

<div class="h-screen flex flex-col bg-base-100">
    <Navbar/>
    <div class="flex-1 overflow-y-auto">
        {@render children()}
    </div>
</div>



