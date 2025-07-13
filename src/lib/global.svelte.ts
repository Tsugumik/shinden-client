import type {User} from "$lib/types";
import {invoke} from "@tauri-apps/api/core";

export enum LoadingState {
    LOADING,
    WARNING,
    ERROR,
    OK
}

export const globalStates: {
    loadingState: LoadingState;
    consoleState: boolean;
    user: User;
} = $state({
    loadingState: LoadingState.OK,
    consoleState: false,
    user: {
        name: null,
        image_url: null
    }
});

export const params: {
    animeName: string;
    seriesUrl: string;
    playersUrl: string;
    playerId: string;
} = $state({
    animeName: "",
    seriesUrl: "",
    playersUrl: "",
    playerId: "",
})

export async function getUserData(): Promise<boolean> {
    const username = await invoke("get_user_name");
    const user_profile_image_url = await invoke("get_user_profile_image");

    if(user_profile_image_url && username) {
        globalStates.user.name = username as string;
        globalStates.user.image_url = user_profile_image_url as string;
        return true;
    }

    return false;
}