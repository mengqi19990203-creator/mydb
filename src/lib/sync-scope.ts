import { computed, ref, watch, type Ref } from "vue";
import * as api from "./api";
import type { ConnectionConfig, ConnectionContext } from "./types";

interface SyncScopeOptions {
  connections: Ref<ConnectionConfig[]>;
  sourceId: Ref<string>;
  targetId: Ref<string>;
  onError?: (message: string) => void;
  /// Allow a target connection with a different SQL driver (data transfer).
  crossDriver?: boolean;
}

const MYSQL_SYSTEM_DATABASES = new Set([
  "information_schema",
  "mysql",
  "performance_schema",
  "sys",
]);

function visibleDatabases(
  driver: string,
  databases: string[],
  current: string | null | undefined,
): string[] {
  if (driver !== "mysql") return databases;
  return databases.filter(
    (database) =>
      !MYSQL_SYSTEM_DATABASES.has(database) || database === current,
  );
}

/**
 * Shared source/target scope handling for the structure and data sync
 * dialogs. Both sides can select a database (MySQL) or database + schema
 * (PostgreSQL) independently, including two scopes inside the same connection.
 */
export function useSyncScope(options: SyncScopeOptions) {
  const sourceContext = ref<ConnectionContext | null>(null);
  const targetContext = ref<ConnectionContext | null>(null);
  const sourceDatabases = ref<string[]>([]);
  const sourceSchemas = ref<string[]>([]);
  const sourceDatabase = ref("");
  const sourceSchema = ref("");
  const targetDatabases = ref<string[]>([]);
  const targetSchemas = ref<string[]>([]);
  const targetDatabase = ref("");
  const targetSchema = ref("");
  const loading = ref(false);
  let refreshToken = 0;
  let sourceSchemaToken = 0;
  let targetSchemaToken = 0;

  const available = computed(() =>
    options.connections.value.filter((connection) =>
      ["sqlite", "mysql", "postgres"].includes(connection.driver),
    ),
  );
  const sourceConnection = computed(
    () => available.value.find((item) => item.id === options.sourceId.value) ?? null,
  );
  const driver = computed(() => sourceConnection.value?.driver ?? "");
  const targetConnection = computed(
    () => available.value.find((item) => item.id === options.targetId.value) ?? null,
  );
  const targetDriver = computed(() => targetConnection.value?.driver ?? "");
  const sameConnectionSupported = computed(
    () => driver.value === "mysql" || driver.value === "postgres",
  );
  const targetOptions = computed(() => {
    const others = available.value.filter(
      (connection) =>
        connection.id !== options.sourceId.value &&
        (options.crossDriver ||
          connection.driver === sourceConnection.value?.driver),
    );
    if (sameConnectionSupported.value && sourceConnection.value) {
      return [...others, sourceConnection.value];
    }
    return others;
  });
  const usesSameConnection = computed(
    () =>
      Boolean(options.sourceId.value) &&
      options.targetId.value === options.sourceId.value,
  );
  const sourceScopeValid = computed(() => {
    if (!options.sourceId.value) return false;
    if (driver.value === "mysql") return Boolean(sourceDatabase.value);
    if (driver.value === "postgres") {
      return Boolean(sourceDatabase.value) && Boolean(sourceSchema.value);
    }
    return true;
  });
  const sourceContextDiffers = computed(() => {
    const context = sourceContext.value;
    if (!context) return false;
    if (driver.value === "mysql") {
      return sourceDatabase.value !== (context.database ?? "");
    }
    if (driver.value === "postgres") {
      return (
        sourceDatabase.value !== (context.database ?? "") ||
        sourceSchema.value !== (context.schema ?? "")
      );
    }
    return false;
  });
  const targetScopeValid = computed(() => {
    if (!options.targetId.value) return false;
    if (options.targetId.value !== options.sourceId.value) {
      if (targetDriver.value === "mysql") return Boolean(targetDatabase.value);
      if (targetDriver.value === "postgres") {
        return Boolean(targetDatabase.value) && Boolean(targetSchema.value);
      }
      return true;
    }
    if (targetDriver.value === "mysql") {
      return Boolean(targetDatabase.value);
    }
    if (targetDriver.value === "postgres") {
      return Boolean(targetDatabase.value) && Boolean(targetSchema.value);
    }
    return false;
  });
  const targetScopeEqualsSource = computed(
    () =>
      usesSameConnection.value &&
      Boolean(targetDatabase.value) &&
      targetDatabase.value === sourceDatabase.value &&
      (driver.value !== "postgres" || targetSchema.value === sourceSchema.value),
  );
  const targetContextDiffers = computed(() => {
    const context = targetContext.value;
    if (!context) return false;
    if (targetDriver.value === "mysql") {
      return targetDatabase.value !== (context.database ?? "");
    }
    if (targetDriver.value === "postgres") {
      return (
        targetDatabase.value !== (context.database ?? "") ||
        targetSchema.value !== (context.schema ?? "")
      );
    }
    return false;
  });

  function optionLabel(connection: ConnectionConfig): string {
    if (connection.id === options.sourceId.value) {
      return `${connection.name} · 当前连接（跨库 / 模式）`;
    }
    return `${connection.name} · ${connection.driver}`;
  }

  function reportError(reason: unknown) {
    options.onError?.(String(reason));
  }

  async function loadSourceSchemas() {
    const currentToken = ++sourceSchemaToken;
    if (driver.value !== "postgres" || !sourceDatabase.value) {
      sourceSchemas.value = [];
      sourceSchema.value = "";
      return;
    }
    try {
      const schemas = await api.listContextSchemas(
        options.sourceId.value,
        sourceDatabase.value,
      );
      if (currentToken !== sourceSchemaToken) return;
      sourceSchemas.value = schemas;
      if (!schemas.includes(sourceSchema.value)) {
        const context = sourceContext.value;
        const sameDatabase = sourceDatabase.value === context?.database;
        const currentSchema = context?.schema ?? "";
        const preferred =
          sameDatabase && schemas.includes(currentSchema)
            ? currentSchema
            : schemas.find((schema) => !sameDatabase || schema !== currentSchema) ??
              schemas[0] ??
              "";
        sourceSchema.value = preferred;
      }
    } catch (reason) {
      if (currentToken === sourceSchemaToken) reportError(reason);
    }
  }

  async function loadTargetSchemas() {
    const currentToken = ++targetSchemaToken;
    const targetId = options.targetId.value;
    if (targetDriver.value !== "postgres" || !targetDatabase.value || !targetId) {
      targetSchemas.value = [];
      targetSchema.value = "";
      return;
    }
    try {
      const schemas = await api.listContextSchemas(
        targetId,
        targetDatabase.value,
      );
      if (currentToken !== targetSchemaToken || targetId !== options.targetId.value) {
        return;
      }
      targetSchemas.value = schemas;
      if (!schemas.includes(targetSchema.value)) {
        const context = targetContext.value;
        const sameDatabase =
          targetDatabase.value === (context?.database ?? "");
        const sameInstance = targetId === options.sourceId.value;
        const forbidden =
          sameInstance && targetDatabase.value === sourceDatabase.value
            ? sourceSchema.value
            : "";
        const preferred = sameDatabase ? context?.schema ?? "" : "";
        targetSchema.value =
          preferred && preferred !== forbidden && schemas.includes(preferred)
            ? preferred
            : schemas.find((schema) => !forbidden || schema !== forbidden) ??
              schemas[0] ??
              "";
      }
    } catch (reason) {
      if (currentToken === targetSchemaToken) reportError(reason);
    }
  }

  async function refresh() {
    const currentToken = ++refreshToken;
    const sourceId = options.sourceId.value;
    const targetId = options.targetId.value;
    if (!sourceId || !targetId) {
      sourceContext.value = null;
      targetContext.value = null;
      sourceDatabases.value = [];
      sourceSchemas.value = [];
      sourceDatabase.value = "";
      sourceSchema.value = "";
      targetDatabases.value = [];
      targetSchemas.value = [];
      targetDatabase.value = "";
      targetSchema.value = "";
      loading.value = false;
      return;
    }

    loading.value = true;
    try {
      const context = await api.getConnectionContext(sourceId);
      if (currentToken !== refreshToken) return;
      sourceContext.value = context;

      if (driver.value === "mysql") {
        const allDatabases = await api.listDatabases(sourceId);
        if (currentToken !== refreshToken) return;
        const databases = visibleDatabases(
          driver.value,
          allDatabases,
          context.database,
        );
        sourceDatabases.value = databases;
        sourceSchemas.value = [];
        sourceSchema.value = "";
        if (!databases.includes(sourceDatabase.value)) {
          sourceDatabase.value =
            context.database && databases.includes(context.database)
              ? context.database
              : databases[0] ?? "";
        }
      } else if (driver.value === "postgres") {
        const databases = await api.listDatabases(sourceId);
        if (currentToken !== refreshToken) return;
        sourceDatabases.value = databases;
        if (!databases.includes(sourceDatabase.value)) {
          sourceDatabase.value =
            context.database && databases.includes(context.database)
              ? context.database
              : databases[0] ?? "";
        }
        await loadSourceSchemas();
      } else {
        sourceDatabases.value = [];
        sourceSchemas.value = [];
        sourceDatabase.value = "";
        sourceSchema.value = "";
      }

      if (targetId !== sourceId) {
        const targetCtx = await api.getConnectionContext(targetId);
        if (currentToken !== refreshToken) return;
        targetContext.value = targetCtx;
        if (targetDriver.value === "mysql") {
          const allDatabases = await api.listDatabases(targetId);
          if (currentToken !== refreshToken) return;
          const databases = visibleDatabases(
            targetDriver.value,
            allDatabases,
            targetCtx.database,
          );
          targetDatabases.value = databases;
          targetSchemas.value = [];
          targetSchema.value = "";
          if (!databases.includes(targetDatabase.value)) {
            targetDatabase.value =
              targetCtx.database && databases.includes(targetCtx.database)
                ? targetCtx.database
                : databases[0] ?? "";
          }
        } else if (targetDriver.value === "postgres") {
          const databases = await api.listDatabases(targetId);
          if (currentToken !== refreshToken) return;
          targetDatabases.value = databases;
          if (!databases.includes(targetDatabase.value)) {
            targetDatabase.value =
              targetCtx.database && databases.includes(targetCtx.database)
                ? targetCtx.database
                : databases[0] ?? "";
          }
          await loadTargetSchemas();
        } else {
          targetDatabases.value = [];
          targetSchemas.value = [];
          targetDatabase.value = "";
          targetSchema.value = "";
        }
        return;
      }

      targetContext.value = context;
      if (targetDriver.value === "mysql") {
        targetDatabases.value = sourceDatabases.value;
        targetSchemas.value = [];
        targetSchema.value = "";
        if (!targetDatabases.value.includes(targetDatabase.value)) {
          targetDatabase.value =
            targetDatabases.value.find(
              (database) => database !== sourceDatabase.value,
            ) ??
            targetDatabases.value[0] ??
            "";
        }
      } else if (targetDriver.value === "postgres") {
        targetDatabases.value = sourceDatabases.value;
        if (!targetDatabases.value.includes(targetDatabase.value)) {
          targetDatabase.value = targetDatabases.value.includes(
            sourceDatabase.value,
          )
            ? sourceDatabase.value
            : targetDatabases.value[0] ?? "";
        }
        await loadTargetSchemas();
      } else {
        targetDatabases.value = [];
        targetSchemas.value = [];
        targetDatabase.value = "";
        targetSchema.value = "";
      }
    } catch (reason) {
      if (currentToken === refreshToken) {
        sourceContext.value = null;
        targetContext.value = null;
        sourceDatabases.value = [];
        sourceSchemas.value = [];
        sourceDatabase.value = "";
        sourceSchema.value = "";
        targetDatabases.value = [];
        targetSchemas.value = [];
        targetDatabase.value = "";
        targetSchema.value = "";
        reportError(reason);
      }
    } finally {
      if (currentToken === refreshToken) loading.value = false;
    }
  }

  watch(options.sourceId, () => {
    sourceDatabase.value = "";
    sourceSchema.value = "";
    targetDatabase.value = "";
    targetSchema.value = "";
    targetContext.value = null;
    if (!targetOptions.value.some((item) => item.id === options.targetId.value)) {
      options.targetId.value = targetOptions.value[0]?.id ?? "";
    }
    void refresh();
  });

  watch(options.targetId, () => {
    targetDatabase.value = "";
    targetSchema.value = "";
    targetContext.value = null;
    void refresh();
  });

  watch(sourceDatabase, () => {
    void loadSourceSchemas();
    if (usesSameConnection.value) {
      void loadTargetSchemas();
    }
  });

  watch(sourceSchema, () => {
    if (usesSameConnection.value && targetDriver.value === "postgres") {
      void loadTargetSchemas();
    }
  });

  watch(targetDatabase, () => {
    void loadTargetSchemas();
  });

  return {
    sourceConnection,
    sourceContext,
    targetContext,
    driver,
    targetDriver,
    sourceDatabases,
    sourceSchemas,
    sourceDatabase,
    sourceSchema,
    targetOptions,
    targetDatabases,
    targetSchemas,
    targetDatabase,
    targetSchema,
    usesSameConnection,
    sourceScopeValid,
    sourceContextDiffers,
    targetContextDiffers,
    targetScopeEqualsSource,
    targetScopeValid,
    loading,
    optionLabel,
    refresh,
  };
}