
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
}
export const logs: LogEntry[] = $state([]);