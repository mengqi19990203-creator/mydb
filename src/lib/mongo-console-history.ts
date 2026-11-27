export interface MongoConsoleHistoryEntry {
  id: string;
  command: string;
  database: string;
  createdAt: number;
  ok: boolean;
  error?: string;
}

export const MONGO_CONSOLE_HISTORY_LIMIT = 50;

let entries: MongoConsoleHistoryEntry[] = [];

export function listMongoConsoleHistory(): MongoConsoleHistoryEntry[] {
  return [...entries];
}

export function addMongoConsoleHistory(entry: {
  command: string;
  database: string;
  ok: boolean;
  error?: string;
}): void {
  const command = entry.command.trim();
  if (!command) return;
  entries = [
    {
      id: `mongo-history-${Date.now()}-${Math.random().toString(16).slice(2)}`,
      command: command.slice(0, 100 * 1024),
      database: entry.database,
      createdAt: Date.now(),
      ok: entry.ok,
      error: entry.error?.slice(0, 500),
    },
    ...entries,
  ].slice(0, MONGO_CONSOLE_HISTORY_LIMIT);
}

export function clearMongoConsoleHistory(): void {
  entries = [];
}
