<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";
    import {log, LogLevel} from "$lib/logs/logs.svelte";
    import {getUserData, globalStates, LoadingState} from "$lib/global.svelte";

    let { data } = $props();

    console.log(data);

    let email: string = $state("");
    let password: string = $state("");
    async function handleLogin(event: Event) {
        event.preventDefault();

        try {
            log(LogLevel.INFO, "Trying to log in");
            globalStates.loadingState = LoadingState.LOADING;
            await invoke("login", {username: email, password: password});
            await getUserData();
            globalStates.loadingState = LoadingState.OK;
            log(LogLevel.SUCCESS, `Successfully logged in`);
        } catch (error) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Could not log in: ${error}`);
        }
    }

    async function logout() {
        try {
            log(LogLevel.INFO, "Trying to log out");
            globalStates.loadingState = LoadingState.LOADING;
            await invoke("logout");
        } catch (error) {
            globalStates.loadingState = LoadingState.ERROR;
            log(LogLevel.ERROR, `Could not log out: ${error}`);
        }

        globalStates.user.name = null;
        globalStates.user.image_url = null;

        globalStates.loadingState = LoadingState.OK;
        log(LogLevel.SUCCESS, "Log out successfully");
    }
</script>

<div class="hero bg-base-200 h-full">
    {#if !globalStates.user.name}
        <div class="hero-content flex-col">
            <div class="text-center py-6">
                <h1 class="text-5xl font-bold">Logowanie</h1>
                <p>
                    Zalogowanie się na konto w serwisie shinden.pl umożliwi ci oglądanie niedostępnych publicznie anime.
                </p>
                <p>Nie masz konta? <a class="link" target="_blank" href="https://shinden.pl/user/0/register">Zarejestruj się</a></p>
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
        {:else}
            <div class="hero-content flex-col">
                <img
                        src={globalStates.user.image_url}
                        class="max-w-sm rounded-lg shadow-2xl"
                        alt="Avatar"
                />
                <div class="text-center">
                    <h1 class="text-5xl font-bold">Zalogowano jako {globalStates.user.name}</h1>
                    <p class="py-6">
                        W razie problemów z dostępem do listy odcinków - zaloguj się ponownie.
                    </p>
                    <button class="btn btn-neutral" onclick={logout}>Wyloguj się</button>
                </div>
            </div>
    {/if}
</div>