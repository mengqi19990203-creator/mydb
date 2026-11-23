<script setup lang="ts">
import { computed, onBeforeUnmount, onMounted, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import ObjectCategoryGroups from "./ObjectCategoryGroups.vue";
import { writeClipboard } from "../lib/clipboard";
import type { Driver, TableInfo } from "../lib/types";

const props = defineProps<{
  connectionId: string;
  driver: Driver;
  tables: TableInfo[];
  databases: string[];
  activeDatabase: string | null;
  schemas: string[];
  activeSchema: string | null;
  databaseCounts?: Record<string, number>;
  loading: boolean;
  activeTable: string | null;
}>();

const emit = defineEmits<{
  select: [table: TableInfo];
  "select-structure": [table: TableInfo];
  "select-index": [table: TableInfo];
  "open-new-tab": [table: TableInfo];
  "import-data": [table: TableInfo];
  "select-database": [database: string];
  "select-schema": [schema: string];
  refresh: [];
  create: [];
  "generate-sql": [payload: { sql: string; title: string }];
}>();

const search = ref("");
const expandedDatabase = ref<string | null>(null);
const collapsedSchemas = reactive<Record<string, boolean>>({});
const nodeMenu = ref<{
  x: number;
  y: number;
  kind: "database" | "schema";
  name: string;
} | null>(null);
const toast = ref("");
let toastTimer = 0;

const isPostgres = computed(() => props.driver === "postgres");

const categoryKey = computed(
  () =>
    `${props.connectionId}:${props.activeDatabase ?? ""}:${
      props.activeSchema ?? ""
    }`,
);

function toggleDatabase(database: string) {
  if (database !== props.activeDatabase) {
    expandedDatabase.value = database;
    emit("select-database", database);
    return;
  }
  expandedDatabase.value =
    expandedDatabase.value === database ? null : database;
}

function schemaExpanded(schema: string): boolean {
  return schema === props.activeSchema && !collapsedSchemas[schema];
}

function onSchemaClick(schema: string) {
  if (schema !== props.activeSchema) {
    emit("select-schema", schema);
    return;
  }
  collapsedSchemas[schema] = !collapsedSchemas[schema];
}

function flashToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 1800);
}

function openNodeMenu(
  event: MouseEvent,
  kind: "database" | "schema",
  name: string,
) {
  event.preventDefault();
  event.stopPropagation();
  nodeMenu.value = {
    x: Math.max(8, Math.min(event.clientX, window.innerWidth - 220)),
    y: Math.max(8, Math.min(event.clientY, window.innerHeight - 230)),
    kind,
    name,
  };
}

function closeNodeMenu() {
  nodeMenu.value = null;
}

function nodeMenuSwitch() {
  const menu = nodeMenu.value;
  closeNodeMenu();
  if (!menu) return;
  if (menu.kind === "database") {
    emit("select-database", menu.name);
  } else {
    emit("select-schema", menu.name);
  }
}

function nodeMenuCreate() {
  closeNodeMenu();
  emit("create");
}

function nodeMenuRefresh() {
  closeNodeMenu();
  emit("refresh");
}

async function nodeMenuCopyName() {
  const menu = nodeMenu.value;
  closeNodeMenu();
  if (!menu) return;
  try {
    await writeClipboard(menu.name);
    flashToast("已复制名称");
  } catch {
    flashToast("复制失败");
  }
}

function onWindowClick() {
  closeNodeMenu();
}

function onWindowKey(event: KeyboardEvent) {
  if (event.key === "Escape") closeNodeMenu();
}

function onTreeKeydown(event: KeyboardEvent) {
  const list = event.currentTarget as HTMLElement;
  const target = event.target as HTMLElement;
  if (!target.matches("[data-tree-nav]")) return;
  const nodes = Array.from(
    list.querySelectorAll<HTMLElement>("[data-tree-nav]"),
  ).filter((node) => node.offsetParent !== null);
  const index = nodes.indexOf(target);
  if (index < 0) return;

  switch (event.key) {
    case "ArrowDown":
      event.preventDefault();
      nodes[(index + 1) % nodes.length]?.focus();
      break;
    case "ArrowUp":
      event.preventDefault();
      nodes[(index - 1 + nodes.length) % nodes.length]?.focus();
      break;
    case "Home":
      event.preventDefault();
      nodes[0]?.focus();
      break;
    case "End":
      event.preventDefault();
      nodes[nodes.length - 1]?.focus();
      break;
    case "ArrowRight":
      event.preventDefault();
      target.click();
      break;
    case "ArrowLeft": {
      event.preventDefault();
      if (target.getAttribute("aria-expanded") === "true") {
        target.click();
      } else {
        const parent = target.parentElement?.closest("[data-tree-nav]");
        if (parent instanceof HTMLElement && parent !== target) {
          parent.focus();
        }
      }
      break;
    }
    case "Enter": {
      event.preventDefault();
      const openButton = target.querySelector<HTMLElement>(".tree-open");
      if (openButton) openButton.click();
      else target.click();
      break;
    }
    default:
      return;
  }

  if (["ArrowDown", "ArrowUp", "Home", "End"].includes(event.key)) {
    const active = document.activeElement;
    if (active instanceof HTMLElement) {
      active.scrollIntoView({ block: "nearest" });
    }
  }
}

onMounted(() => {
  window.addEventListener("click", onWindowClick);
  window.addEventListener("keydown", onWindowKey);
  window.addEventListener("resize", closeNodeMenu);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", onWindowClick);
  window.removeEventListener("keydown", onWindowKey);
  window.removeEventListener("resize", closeNodeMenu);
  window.clearTimeout(toastTimer);
});

watch(
  () => props.activeDatabase,
  (database) => {
    if (database) {
      expandedDatabase.value = database;
    }
  },
  { immediate: true },
);

watch(
  () => props.activeSchema,
  (schema) => {
    if (schema) {
      delete collapsedSchemas[schema];
    }
  },
);

watch(
  () => props.connectionId,
  () => {
    expandedDatabase.value = props.activeDatabase;
    search.value = "";
  },
);

watch(
  () => [props.activeDatabase, props.activeSchema],
  () => {
    search.value = "";
  },
);
</script>

<template>
  <aside class="object-browser">
    <header class="panel-header">
      <span>对象</span>
      <div class="panel-actions">
        <button
          class="icon-button"
          title="新建表"
          aria-label="新建表"
          @click="emit('create')"
        >
          <Icon name="plus" />
        </button>
        <button
          class="icon-button"
          title="刷新对象列表"
          aria-label="刷新对象列表"
          @click="emit('refresh')"
        >
          <Icon name="refresh" />
        </button>
      </div>
    </header>
    <div class="object-search">
      <Icon name="search" :size="14" />
      <input
        v-model="search"
        placeholder="搜索对象…"
        @keydown.esc="search = ''"
      />
      <button
        v-if="search"
        class="icon-button search-clear"
        title="清空搜索"
        aria-label="清空搜索"
        @click="search = ''"
      >
        <Icon name="close" :size="12" />
      </button>
    </div>
    <div class="object-list" @keydown="onTreeKeydown">
      <div v-if="loading && !databases.length" class="panel-empty">
        <span class="spinner"></span>
        加载中…
      </div>

      <template v-else-if="databases.length">
        <div
          v-for="database in databases"
          :key="database"
          class="tree-database"
        >
          <button
            class="object-group-header tree-database-header"
            data-tree-nav
            :aria-expanded="expandedDatabase === database"
            :class="{ active: database === activeDatabase }"
            :title="
              database === activeDatabase
                ? '展开或收起当前数据库'
                : `切换到数据库 ${database}`
            "
            @click="toggleDatabase(database)"
            @contextmenu.prevent.stop="
              openNodeMenu($event, 'database', database)
            "
          >
            <Icon
              :name="
                expandedDatabase === database
                  ? 'chevron-down'
                  : 'chevron-right'
              "
              :size="12"
            />
            <Icon class="object-kind" name="database" :size="14" />
            <span class="tree-database-name">{{ database }}</span>
            <span
              v-if="databaseCounts && databaseCounts[database] !== undefined"
              class="group-count"
            >
              {{ databaseCounts[database] }}
            </span>
            <span
              v-else-if="database === activeDatabase"
              class="group-count"
            >
              {{ tables.length }}
            </span>
            <span
              v-if="database === activeDatabase"
              class="database-active"
            >
              当前
            </span>
          </button>

          <div
            v-if="
              database === activeDatabase &&
              expandedDatabase === database
            "
            class="tree-database-body"
          >
            <div v-if="loading" class="tree-hint">加载中…</div>

            <template v-if="isPostgres && schemas.length">
              <div class="tree-schema-section">
                <div class="object-group-header tree-schema-heading">
                  <Icon name="structure" :size="13" />
                  <span>模式</span>
                  <span class="group-count">{{ schemas.length }}</span>
                </div>
                <div
                  v-for="schema in schemas"
                  :key="schema"
                  class="tree-schema"
                >
                  <div
                    class="object-item tree-schema-row"
                    data-tree-nav
                    :aria-expanded="schemaExpanded(schema)"
                    :class="{ active: schema === activeSchema }"
                    role="button"
                    tabindex="0"
                    :title="
                      schema === activeSchema
                        ? '展开或收起当前模式'
                        : `切换到模式 ${schema}`
                    "
                    @click="onSchemaClick(schema)"
                    @contextmenu.prevent.stop="
                      openNodeMenu($event, 'schema', schema)
                    "
                  >
                    <Icon
                      class="tree-chevron"
                      :name="
                        schemaExpanded(schema)
                          ? 'chevron-down'
                          : 'chevron-right'
                      "
                      :size="12"
                    />
                    <Icon
                      class="object-kind"
                      name="structure"
                      :size="14"
                    />
                    <span class="object-name">{{ schema }}</span>
                    <span
                      v-if="schema === activeSchema"
                      class="database-active"
                    >
                      当前
                    </span>
                  </div>

                  <div
                    v-if="schemaExpanded(schema)"
                    class="tree-schema-body"
                  >
                    <ObjectCategoryGroups
                      :key="categoryKey"
                      :connection-id="connectionId"
                      :driver="driver"
                      :tables="tables"
                      :search="search"
                      :active-table="activeTable"
                      :database="database"
                      :schema="schema"
                      @select="emit('select', $event)"
                      @select-structure="emit('select-structure', $event)"
                      @select-index="emit('select-index', $event)"
                      @open-new-tab="emit('open-new-tab', $event)"
                      @import-data="emit('import-data', $event)"
                      @create="emit('create')"
                      @refresh="emit('refresh')"
                      @generate-sql="emit('generate-sql', $event)"
                    />
                  </div>
                </div>
              </div>
            </template>

            <ObjectCategoryGroups
              v-else
              :key="categoryKey"
              :connection-id="connectionId"
              :driver="driver"
              :tables="tables"
              :search="search"
              :active-table="activeTable"
              :database="database"
              @select="emit('select', $event)"
              @select-structure="emit('select-structure', $event)"
              @select-index="emit('select-index', $event)"
              @open-new-tab="emit('open-new-tab', $event)"
              @import-data="emit('import-data', $event)"
              @create="emit('create')"
              @refresh="emit('refresh')"
              @generate-sql="emit('generate-sql', $event)"
            />
          </div>
        </div>
      </template>

      <template v-else>
        <ObjectCategoryGroups
          :key="categoryKey"
          :connection-id="connectionId"
          :driver="driver"
          :tables="tables"
          :search="search"
          :active-table="activeTable"
          :database="activeDatabase ?? 'main'"
          @select="emit('select', $event)"
          @select-structure="emit('select-structure', $event)"
          @select-index="emit('select-index', $event)"
          @open-new-tab="emit('open-new-tab', $event)"
          @import-data="emit('import-data', $event)"
          @create="emit('create')"
          @refresh="emit('refresh')"
          @generate-sql="emit('generate-sql', $event)"
        />
      </template>
    </div>
  </aside>

  <div
    v-if="nodeMenu"
    class="context-menu object-context-menu"
    role="menu"
    :style="{
      left: `${nodeMenu.x}px`,
      top: `${nodeMenu.y}px`,
    }"
    @click.stop
  >
    <button
      class="context-menu-item"
      role="menuitem"
      :disabled="
        nodeMenu.kind === 'database'
          ? nodeMenu.name === activeDatabase
          : nodeMenu.name === activeSchema
      "
      @click="nodeMenuSwitch"
    >
      <Icon name="database" :size="14" />
      <span>
        {{
          nodeMenu.kind === "database"
            ? "切换到此数据库"
            : "切换到此模式"
        }}
      </span>
    </button>
    <button
      class="context-menu-item"
      role="menuitem"
      @click="nodeMenuCreate"
    >
      <Icon name="plus" :size="14" />
      <span>新建表</span>
    </button>
    <div class="context-separator"></div>
    <button
      class="context-menu-item"
      role="menuitem"
      @click="nodeMenuRefresh"
    >
      <Icon name="refresh" :size="14" />
      <span>刷新对象</span>
    </button>
    <button
      class="context-menu-item"
      role="menuitem"
      @click="nodeMenuCopyName"
    >
      <Icon name="copy" :size="14" />
      <span>复制名称</span>
    </button>
  </div>

  <div v-if="toast" class="grid-toast" role="status" aria-live="polite">
    {{ toast }}
  </div>
</template>
