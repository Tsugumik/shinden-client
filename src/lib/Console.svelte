<script>
import {globalStates} from "$lib/global.svelte.js";
import Error from "$lib/logs/Error.svelte";
import {slide} from "svelte/transition";
import {LogLevel, logs} from "$lib/logs/logs.svelte.js";
import Info from "$lib/logs/Info.svelte";
import Warning from "$lib/logs/Warning.svelte";
import Success from "$lib/logs/Success.svelte";

</script>

<div class="fixed flex flex-col h-full w-full bg-base-100/99 top-0 left-0 z-10 font-mono" transition:slide>
    <header data-tauri-drag-region class="navbar shadow-sm bg-base-300 h-3 gap-4">
        <div data-tauri-drag-region class="flex-1 font-mono">
            <a class="btn btn-ghost text-xl" href="/">Console</a>
        </div>
        <div>
            <button class="btn btn-circle btn-sm" onclick={async()=>{globalStates.consoleState = !globalStates.consoleState}}>
                &#x2715;
            </button>
        </div>
    </header>

    <div class="flex gap-4 p-4 flex-1 list scroll max-h-full overflow-y-scroll">
        {#each logs.slice().reverse() as log}
            {#if log.logLevel === LogLevel.INFO}
                <Info message={log.getText()} />
                {:else if log.logLevel === LogLevel.ERROR}
                <Error message={log.getText()} />
                {:else if log.logLevel === LogLevel.WARNING}
                <Warning message={log.getText()} />
                {:else if log.logLevel === LogLevel.SUCCESS}
                <Success message={log.getText()} />
            {/if}
        {/each}
    </div>
</div>