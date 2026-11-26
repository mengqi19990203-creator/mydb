export interface PinnedObject {
  connectionId: string;
  database: string;
  schema: string;
  kind: string;
  name: string;
}

export const OBJECT_PINS_KEY = "mydb.objectPins.v1";
export const OBJECT_PINS_LIMIT = 50;

interface StoredPins {
  version: 1;
  items: PinnedObject[];
}

function scopePart(value: string | null | undefined): string {
  return value?.trim() ?? "";
}

export function pinKey(pin: PinnedObject): string {
  return [
    pin.connectionId,
    pin.database,
    pin.schema,
    pin.kind,
    pin.name,
  ].join("\u0001");
}

export function scopeKey(
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
): string {
  return [connectionId, scopePart(database), scopePart(schema)].join("\u0001");
}

export function makePinnedObject(
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
  kind: string,
  name: string,
): PinnedObject {
  return {
    connectionId,
    database: scopePart(database),
    schema: scopePart(schema),
    kind,
    name,
  };
}

function validPin(value: unknown): PinnedObject | null {
  if (!value || typeof value !== "object") return null;
  const item = value as Record<string, unknown>;
  const fields = ["connectionId", "database", "schema", "kind", "name"];
  if (fields.some((field) => typeof item[field] !== "string")) return null;
  const pin: PinnedObject = {
    connectionId: String(item.connectionId),
    database: String(item.database),
    schema: String(item.schema),
    kind: String(item.kind),
    name: String(item.name),
  };
  if (!pin.connectionId || !pin.kind || !pin.name) return null;
  return pin;
}

/** Reads local pins defensively; corrupt or partial data falls back to []. */
export function loadPins(): PinnedObject[] {
  try {
    const raw = window.localStorage.getItem(OBJECT_PINS_KEY);
    if (!raw) return [];
    const parsed = JSON.parse(raw) as Partial<StoredPins>;
    if (!parsed || parsed.version !== 1 || !Array.isArray(parsed.items)) {
      return [];
    }
    const seen = new Set<string>();
    const pins: PinnedObject[] = [];
    for (const value of parsed.items) {
      const pin = validPin(value);
      if (!pin) continue;
      const key = pinKey(pin);
      if (seen.has(key)) continue;
      seen.add(key);
      pins.push(pin);
      if (pins.length >= OBJECT_PINS_LIMIT) break;
    }
    return pins;
  } catch {
    return [];
  }
}

export function savePins(pins: PinnedObject[]): void {
  try {
    window.localStorage.setItem(
      OBJECT_PINS_KEY,
      JSON.stringify({ version: 1, items: pins.slice(0, OBJECT_PINS_LIMIT) }),
    );
  } catch {
    // Storage may be unavailable; pins remain in memory for this run.
  }
}

export function pinsForScope(
  pins: readonly PinnedObject[],
  connectionId: string,
  database: string | null | undefined,
  schema: string | null | undefined,
): PinnedObject[] {
  const key = scopeKey(connectionId, database, schema);
  return pins.filter(
    (pin) => scopeKey(pin.connectionId, pin.database, pin.schema) === key,
  );
}

export function isPinned(
  pins: readonly PinnedObject[],
  candidate: PinnedObject,
): boolean {
  const key = pinKey(candidate);
  return pins.some((pin) => pinKey(pin) === key);
}

export function togglePinned(
  pins: readonly PinnedObject[],
  candidate: PinnedObject,
): { items: PinnedObject[]; pinned: boolean; limitReached: boolean } {
  const key = pinKey(candidate);
  const exists = pins.some((pin) => pinKey(pin) === key);
  if (exists) {
    return {
      items: pins.filter((pin) => pinKey(pin) !== key),
      pinned: false,
      limitReached: false,
    };
  }
  if (pins.length >= OBJECT_PINS_LIMIT) {
    return { items: [...pins], pinned: false, limitReached: true };
  }
  return {
    items: [...pins, candidate],
    pinned: true,
    limitReached: false,
  };
}