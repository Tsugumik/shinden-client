<script lang="ts">
    import "../app.css";
    import Navbar from "$lib/Navbar.svelte";
    import {invoke} from "@tauri-apps/api/core";
    let { children } = $props();
</script>

<div class="h-screen flex flex-col">
    <Navbar/>
    <div class="flex-1 overflow-y-auto">
        {@render children()}
    </div>
</div>



