<script lang="ts">
    import LoadingButton from "$lib/LoadingButton.svelte";
    import AccountButton from "$lib/AccountButton.svelte";
    import { getCurrentWindow } from "@tauri-apps/api/window";
    import UpdateButton from "$lib/logs/UpdateButton.svelte";
</script>

<header data-tauri-drag-region class="navbar shadow-sm bg-base-300 h-3 gap-4">
    <div data-tauri-drag-region class="flex-1 font-[Orbitron] flex items-center gap-4">
        <a class="btn btn-ghost text-xl" href="/">Shinden Client 4.0</a>
        <LoadingButton />
    </div>
    <div class="flex-none">
        <ul class="menu menu-horizontal px-1 flex items-center">
            <li><AccountButton/></li>
            <li><UpdateButton/></li>
        </ul>
    </div>

    <div>
        <input type="checkbox" value="light" class="toggle theme-controller" />
    </div>

    <div>
        <button class="btn btn-circle btn-sm" onclick={() => history.back()}>
            &#8592;
        </button>
        <button class="btn btn-circle btn-sm" onclick={() => getCurrentWindow().minimize()}>
            —
        </button>
        <button class="btn btn-circle btn-sm" onclick={() => getCurrentWindow().close()}>
            &#x2715;
        </button>
    </div>
</header>