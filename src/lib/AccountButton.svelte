<script lang="ts">
    import {invoke} from "@tauri-apps/api/core";
    import {globalStates} from "$lib/global.svelte";
    import {onMount} from "svelte";

    let loading: boolean = $state(false);

    async function getLogin() {
        loading = true;
        const username = await invoke("get_user_name");
        const user_profile_image_url = await invoke("get_user_profile_image");

        if(user_profile_image_url && username) {
            globalStates.user.name = username as string;
            globalStates.user.image_url = user_profile_image_url as string;

            console.log(user_profile_image_url, username);
        }

        loading = false;
    }

    onMount(async () => {
        await getLogin();
    });
</script>

<a href="/account">
{#if loading}
    <span class="loading loading-dots loading-md"></span>
    {:else }
    {#if globalStates.user.name}
        {globalStates.user.name}
        <div class="avatar">
            <div class="w-8 rounded">
                <img
                        src={globalStates.user.image_url}
                        alt="Avatar"
                />
            </div>
        </div>
        {:else }
        Zaloguj się
    {/if}
{/if}
</a>


