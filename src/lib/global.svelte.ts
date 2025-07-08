import type {User} from "$lib/types";

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
} = $state({
    animeName: "",
    seriesUrl: "",
})