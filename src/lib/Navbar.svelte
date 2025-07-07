<script lang="ts">
    import LoadingButton from "$lib/LoadingButton.svelte";
</script>

<header data-tauri-drag-region class="navbar shadow-sm bg-base-300 h-3 gap-4">
    <div data-tauri-drag-region class="flex-1 font-[Orbitron] flex items-center gap-4">
        <a class="btn btn-ghost text-xl" href="/">Shinden Client 4.0</a>
        <LoadingButton />
    </div>
    <div class="flex-none">
        <ul class="menu menu-horizontal px-1">
            <li>
                <a href="/account">Konto</a></li>
            <li><a href="#">Ustawienia</a></li>
        </ul>
    </div>

    <div>
        <input type="checkbox" value="light" class="toggle theme-controller" />
    </div>

    <div>
        <button class="btn btn-circle btn-sm">
            &#128469;
        </button>
        <button class="btn btn-circle btn-sm">
            &#x2715;
        </button>
    </div>
</header>