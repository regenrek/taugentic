export interface DaemonProcessEntry {
  command: string;
  pid: number;
}

export function isDaemonCommand(command: string): boolean;

export function parseDaemonProcessEntries(processTable: string): DaemonProcessEntry[];
