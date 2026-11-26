export interface PendingInsertSnapshot {
  id: number;
  values: Array<unknown | undefined>;
}

/** A copy of the table editor's uncommitted changes. */
export interface PendingSnapshot {
  updates: Record<number, Record<string, unknown>>;
  deletes: number[];
  inserts: PendingInsertSnapshot[];
}

export function clonePendingSnapshot(snapshot: PendingSnapshot): PendingSnapshot {
  const updates: Record<number, Record<string, unknown>> = {};
  for (const [key, overlay] of Object.entries(snapshot.updates)) {
    updates[Number(key)] = { ...overlay };
  }
  return {
    updates,
    deletes: [...snapshot.deletes],
    inserts: snapshot.inserts.map((draft) => ({
      id: draft.id,
      values: [...draft.values],
    })),
  };
}

/** Pushes a copy of `snapshot` and trims the oldest entries past `limit`. */
export function pushPendingHistory(
  stack: PendingSnapshot[],
  snapshot: PendingSnapshot,
  limit = 100,
): void {
  stack.push(clonePendingSnapshot(snapshot));
  if (stack.length > limit) stack.splice(0, stack.length - limit);
}

/** Pops the newest snapshot (already cloned when it was pushed). */
export function takePendingHistory(
  stack: PendingSnapshot[],
): PendingSnapshot | undefined {
  return stack.pop();
}