import {check} from "@tauri-apps/plugin-updater";

export enum UpdateState {
    CHECKING,
    ERROR,
    AVAILABLE,
    NOT_AVAILABLE,
    DOWNLOADING,
    INSTALLED,
    UNKNOWN
}

export const status: { updateState: UpdateState, statusMessage: string } = $state({
    updateState: UpdateState.UNKNOWN,
    statusMessage: getStatusMessage( UpdateState.UNKNOWN)
});


export function getStatusMessage(state: UpdateState) {
    switch(state) {
        case UpdateState.CHECKING :
            return "Sprawdzanie aktualizacji";
        case UpdateState.ERROR:
            return "Wystąpił błąd podczas sprawdzania aktualizacji";
        case UpdateState.AVAILABLE:
            return "Dostępna jest nowa wersja";
        case UpdateState.NOT_AVAILABLE:
            return "Brak dostępnych aktualizacji";
        case UpdateState.DOWNLOADING:
            return "Pobieranie aktualizacji";
        case UpdateState.INSTALLED:
            return "Zainstalowano aktualizację";
        case UpdateState.UNKNOWN:
            return "Nie sprawdzano aktualizacji";
        default:
            throw new Error("Unknown state");
    }
}

export async function setUpdateState(state: UpdateState) {
    status.updateState = state;
    status.statusMessage = getStatusMessage(state);
}

export async function checkUpdate() : Promise<boolean> {
    await setUpdateState(UpdateState.CHECKING);
    try {
        const update = await check();
        if(update) {
            await setUpdateState(UpdateState.AVAILABLE);
            return true;
        } else {
            await setUpdateState(UpdateState.NOT_AVAILABLE);
            return false;
        }
    } catch (e) {
        await setUpdateState(UpdateState.ERROR);
        throw(e);
    }
}