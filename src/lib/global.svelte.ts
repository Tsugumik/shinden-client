
export enum LoadingState {
    LOADING,
    WARNING,
    ERROR,
    OK
}

export const globalStates = $state({
    loadingState: LoadingState.OK,
    consoleState: false,
});

export const params = $state({
    animeName: "",
    seriesUrl: "",
})