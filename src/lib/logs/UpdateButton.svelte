<script lang="ts">
    import {checkUpdate, status, UpdateState} from "$lib/updater.svelte";
    import {onMount} from "svelte";

    let statusClass: string = $state("");

    $effect(()=> {
        switch(status.updateState) {
            case UpdateState.CHECKING :
                statusClass = "status-info";
                break;
            case UpdateState.ERROR:
                statusClass = "status-error";
                break;
            case UpdateState.AVAILABLE:
                statusClass = "status-accent";
                break;
            case UpdateState.NOT_AVAILABLE:
                statusClass = "status-success";
                break;
            case UpdateState.DOWNLOADING:
                statusClass = "status-secondary";
                break;
            case UpdateState.INSTALLED:
                statusClass = "status-primary";
                break;
            case UpdateState.UNKNOWN:
                statusClass = "status-warning";
                break;
            default:
                statusClass = "status-neutral";
                break;

        }
    })

    onMount(async ()=> {
        await checkUpdate();
    });
</script>

<a href="/info">
    Aktualizacje<span aria-label="success" class="status {statusClass}"></span>
</a>