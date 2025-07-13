<script lang="ts">
    import type {Player} from "$lib/types";
    import {params} from "$lib/global.svelte";
    import {goto} from "$app/navigation";

    let { player, iterator } : {
        player: Player, iterator: number
    } = $props();

    async function handleButton(playerId: string) {
        params.playerId = playerId;
        await goto(`/watching`);
    }
</script>
        <li class="list-row flex items-center justify-between">
            <div class="text-4xl font-thin opacity-30 tabular-nums w-16 text-center">{iterator+1}</div>
            <div class="list-col-grow flex-1">
                <div>Język napisów: {player.lang_subs}</div>
                <div>Język audio: {player.lang_audio}</div>
            </div>
            <div class="text-4xl font-thin opacity-30 tabular-nums w-28 text-center">{player.max_res}</div>
            <button class="btn btn-square btn-ghost" aria-label="play" onclick={async() => {await handleButton(player.online_id)}}>
                <svg class="size-[1.2em]" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"><g stroke-linejoin="round" stroke-linecap="round" stroke-width="2" fill="none" stroke="currentColor"><path d="M6 3L20 12 6 21 6 3z"></path></g></svg>
            </button>
        </li>