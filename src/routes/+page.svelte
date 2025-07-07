<script lang="ts">
    import {globalStates, LoadingState, params} from "$lib/global.svelte.js";
    import {onMount} from "svelte";
    import {invoke} from "@tauri-apps/api/core";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {goto} from "$app/navigation";


    let animeName: string = $state("");

    globalStates.loadingState = LoadingState.LOADING;

    onMount(async () => {
        try {
            log(LogLevel.INFO, "Testing connection to http://shinden.pl");
            await invoke("test_connection");
            globalStates.loadingState = LoadingState.OK;
            log(LogLevel.SUCCESS, "Connection to http://shinden.pl established");
        } catch (error) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, "Error connection to http://shinden.pl");
        }
    });

    function handleButton(event: Event) {
        event.preventDefault();
        goto(`/search/${animeName}`);
    }

</script>

<div class="hero bg-base-200 min-h-full">
    <div class="hero-content flex-row">
        <img
                src="/bg.jpg"
                class="max-w-sm rounded-lg shadow-2xl"
                alt="anime"
        />
        <div>
            <h1 class="text-5xl font-bold">Wyszukaj ulubione anime</h1>
            <p class="py-6">
                Na co masz dziś ochotę?
            </p>

            <form class="join w-full" onsubmit={handleButton}>
                <label class="input join-item w-full">
                    <svg class="h-[1em] opacity-50" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24">
                        <g
                                stroke-linejoin="round"
                                stroke-linecap="round"
                                stroke-width="2.5"
                                fill="none"
                                stroke="currentColor"
                        >
                            <circle cx="11" cy="11" r="8"></circle>
                            <path d="m21 21-4.3-4.3"></path>
                        </g>
                    </svg>
                    <input type="search" required bind:value={animeName}/>
                </label>
                <button class="btn btn-primary join-item">Szukaj</button>
            </form>
        </div>
    </div>
</div>