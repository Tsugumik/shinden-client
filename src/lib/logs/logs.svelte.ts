
import { invoke } from "@tauri-apps/api/core";

export enum LogLevel {
    ERROR,
    INFO,
    SUCCESS,
    WARNING
}
export class LogEntry {
    public text: string;
    public date: string;
    public logLevel: LogLevel;
    constructor(_logLevel: LogLevel, _message: string) {
        this.logLevel = _logLevel;
        this.text = _message;
        let currentdate = new Date();
        let datetime = currentdate.getDate() + "/" + (currentdate.getMonth()+1)  + "/" + currentdate.getFullYear() + " @ "  + currentdate.getHours() + ":"  + currentdate.getMinutes() + ":" + currentdate.getSeconds();
        this.date = datetime;
    }

    public getText() {
        return `[${this.date}] - [${this.text}]`
    }
}

export function log(level: LogLevel, message: string) {
    logs.push(new LogEntry(level, message));
    try {
        void invoke("write_log", { level: LogLevel[level], message }).catch((error) => {
            console.error("Could not write project log", error);
        });
    } catch (error) {
        console.error("Could not write project log", error);
    }
}
export const logs: LogEntry[] = $state([]);
