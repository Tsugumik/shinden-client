<script lang="ts">
    import type {Player} from "$lib/types";
    import PlayerListElement from "$lib/PlayerListElement.svelte";

    let { children, keys, group } : { children: any, keys: string[], group: Record<string, Player[]>} = $props();

</script>

<div class="flex flex-col gap-4 p-4">
    <div class="collapse collapse-arrow bg-base-100 border-base-300 border">
        <input type="checkbox" />
        <div class="collapse-title collapse-arrow flex gap-2">
            <div class="badge badge-info">{keys.length}</div>{@render children()}
        </div>
        <div class="collapse-content text-sm">
            <ul class="list flex gap-2">
                {#each keys as playerKey}
                    <div class="collapse collapse-arrow bg-base-100 border-base-300 border">
                        <input type="checkbox" />
                        <div class="collapse-title collapse-arrow font-bold">{playerKey}</div>
                        <div class="collapse-content text-sm">
                            {#each group[playerKey] as player, i}
                                <ul class="list">
                                    <PlayerListElement player={player} iterator={i}/>
                                </ul>
                            {/each}
                        </div>
                    </div>
                {/each}
            </ul>
        </div>
    </div>
</div>

