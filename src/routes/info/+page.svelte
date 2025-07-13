<script lang="ts">
    import {app} from "@tauri-apps/api";
    import {onMount} from "svelte";
    import {checkUpdate, getAndinstallUpdate, status, UpdateState} from "$lib/updater.svelte";
    import {globalStates, LoadingState} from "$lib/global.svelte";
    import {log, LogLevel} from "$lib/logs/logs.svelte";

    let version: string | undefined = $state();

    onMount(async() => {
        version = await app.getVersion();
    });

    async function checkForUpdates() {
        globalStates.loadingState = LoadingState.LOADING;
        log(LogLevel.INFO, "Checking for updates...")

        try {

            let status = await checkUpdate();

            if (status) {
                log(LogLevel.SUCCESS, "New update available!");
            } else {
                log(LogLevel.INFO, "No updates available!");
            }
            globalStates.loadingState = LoadingState.OK;


        } catch (e) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `${e}`);
        }
    }

    async function installUpdate() {
        await getAndinstallUpdate();
    }
</script>

<div class="flex flex-col items-center gap-2 py-2">
    <h2 class="text-center text-lg">Aktualizacje</h2>
    <div class="badge badge-dash w-96">{status.statusMessage}</div>
    <div class="join">
        <button class="btn join-item" onclick={checkForUpdates}>Sprawdź dostępność</button>
        <button class="btn join-item" disabled={!(status.updateState===UpdateState.AVAILABLE)} onclick={getAndinstallUpdate}>Pobierz i zainstaluj</button>
    </div>

</div>

<div class="divider px-4"></div>
<div class="hero">
    <div class="hero-overlay bg-base-100/90"></div>
    <div class="hero-content text-center">
        <div class="max-w-md flex flex-col gap-2 items-center">
            <div>
                <h1 class="text-5xl font-bold font-[Orbitron] text-nowrap">Shinden Client 4</h1>
                <h2>v.{version}</h2>
                <p class="font-mono">
                    MIT License
                </p>
            </div>
            <div>
                <div class="font-mono">Thanks to</div>
                <div class="flex flex-row justify-around p-4 gap-10">
                    <a href="https://vitejs.dev" target="_blank" class="drop-shadow-sm drop-shadow-base-content">
                        <img src="/vite.svg" class="w-14" alt="Vite Logo" />
                    </a>
                    <a href="https://tauri.app" target="_blank" class="drop-shadow-sm drop-shadow-base-content">
                        <img src="/tauri.svg" class="w-14" alt="Tauri Logo" />
                    </a>
                    <a href="https://kit.svelte.dev" target="_blank" class="drop-shadow-sm drop-shadow-base-content">
                        <img src="/svelte.svg" class="w-14" alt="SvelteKit Logo" />
                    </a>
                </div>
            </div>
            <a target="_blank" href="https://github.com/Tsugumik"><img src="/Badge.svg" width="200" alt="Badge" class="drop-shadow-sm drop-shadow-base-content"></a>
        </div>
    </div>
</div>

