<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {globalStates, LoadingState} from "$lib/global.svelte";

    let email: string = $state("");
    let password: string = $state("");
    async function handleLogin(event: Event) {
        event.preventDefault();

        try {
            log(LogLevel.INFO, "Trying to log in");
            globalStates.loadingState = LoadingState.LOADING;
            await invoke("login", {username: email, password: password});
        } catch (error) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Could not log in: ${error}`);
        }
    }
</script>



<div class="hero bg-base-200 h-full">
    <div class="hero-content flex-col">
        <div class="text-center lg:text-left">
            <h1 class="text-5xl font-bold">Logowanie</h1>
            <p class="py-6">
                Zalogowanie się na konto w serwisie shinden.pl umożliwi ci oglądanie niedostępnych publicznie anime.
            </p>
        </div>
        <div class="card bg-base-100 w-full max-w-sm shrink-0 shadow-2xl">
            <form class="card-body" onsubmit={handleLogin}>
                <fieldset class="fieldset">
                    <label class="label">Email</label>
                    <input type="email" class="input" placeholder="Email" required bind:value={email} />
                    <label class="label">Hasło</label>
                    <input type="password" class="input" placeholder="Hasło" required bind:value={password} />
                    <button class="btn btn-neutral mt-4">Zaloguj się</button>
                </fieldset>
            </form>
        </div>
    </div>
</div>