<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from "vue";
import ConfirmDialog from "./ConfirmDialog.vue";
import Icon from "./Icon.vue";
import {
  MAX_PROFILES_PER_TABLE,
  MAX_PROFILE_NAME_LENGTH,
  type TableViewProfile,
} from "../lib/table-views";

const props = defineProps<{
  profiles: TableViewProfile[];
  activeId: string | null;
  defaultId: string | null;
  dirty: boolean;
  disabled: boolean;
  scopeLabel: string;
}>();

const emit = defineEmits<{
  apply: [id: string];
  save: [name: string, makeDefault: boolean];
  update: [];
  rename: [id: string, name: string];
  remove: [id: string];
  "set-default": [id: string | null];
}>();

const hostRef = ref<HTMLElement | null>(null);
const nameInput = ref<HTMLInputElement | null>(null);
const open = ref(false);
const manage = ref(false);
const error = ref("");
const prompt = ref<{
  kind: "save" | "rename";
  id: string | null;
  value: string;
  makeDefault: boolean;
} | null>(null);
const deleteTarget = ref<TableViewProfile | null>(null);

const activeProfile = computed(
  () => props.profiles.find((profile) => profile.id === props.activeId) ?? null,
);
const buttonLabel = computed(() =>
  activeProfile.value ? activeProfile.value.name : "视图",
);
const limitReached = computed(
  () => props.profiles.length >= MAX_PROFILES_PER_TABLE,
);

function toggleMenu() {
  if (props.disabled) return;
  open.value = !open.value;
  if (!open.value) manage.value = false;
  error.value = "";
}

function closeMenu() {
  open.value = false;
  manage.value = false;
}

function applyProfile(profile: TableViewProfile) {
  closeMenu();
  emit("apply", profile.id);
}

function startSave() {
  error.value = "";
  prompt.value = {
    kind: "save",
    id: null,
    value: activeProfile.value ? `${activeProfile.value.name} 副本` : "",
    makeDefault: false,
  };
  void nextTick(() => nameInput.value?.focus());
}

function startRename(profile: TableViewProfile) {
  error.value = "";
  prompt.value = {
    kind: "rename",
    id: profile.id,
    value: profile.name,
    makeDefault: false,
  };
  void nextTick(() => {
    nameInput.value?.focus();
    nameInput.value?.select();
  });
}

function cancelPrompt() {
  prompt.value = null;
  error.value = "";
}

function submitPrompt() {
  const current = prompt.value;
  if (!current) return;
  const name = current.value.trim();
  if (!name) {
    error.value = "请输入视图名称";
    return;
  }
  if (name.length > MAX_PROFILE_NAME_LENGTH) {
    error.value = `名称最多 ${MAX_PROFILE_NAME_LENGTH} 个字符`;
    return;
  }
  const duplicate = props.profiles.some(
    (profile) =>
      profile.id !== current.id &&
      profile.name.toLocaleLowerCase() === name.toLocaleLowerCase(),
  );
  if (duplicate) {
    error.value = "当前表已有同名视图";
    return;
  }
  if (current.kind === "save") {
    if (limitReached.value) {
      error.value = `每张表最多 ${MAX_PROFILES_PER_TABLE} 个视图`;
      return;
    }
    emit("save", name, current.makeDefault);
  } else if (current.id) {
    emit("rename", current.id, name);
  }
  prompt.value = null;
  error.value = "";
  manage.value = false;
  closeMenu();
}

function updateCurrent() {
  closeMenu();
  emit("update");
}

function requestDelete(profile: TableViewProfile) {
  deleteTarget.value = profile;
}

function confirmDelete() {
  const target = deleteTarget.value;
  deleteTarget.value = null;
  if (target) emit("remove", target.id);
}

function toggleDefault(profile: TableViewProfile) {
  emit("set-default", props.defaultId === profile.id ? null : profile.id);
}

function onDocumentClick(event: MouseEvent) {
  if (!open.value) return;
  const host = hostRef.value;
  if (host && event.target instanceof Node && host.contains(event.target)) {
    return;
  }
  closeMenu();
}

function onKeydown(event: KeyboardEvent) {
  if (event.key !== "Escape") return;
  if (deleteTarget.value) return;
  if (prompt.value) {
    prompt.value = null;
    error.value = "";
    return;
  }
  if (open.value) {
    closeMenu();
  }
}

onMounted(() => {
  window.addEventListener("click", onDocumentClick);
  window.addEventListener("keydown", onKeydown);
});

onBeforeUnmount(() => {
  window.removeEventListener("click", onDocumentClick);
  window.removeEventListener("keydown", onKeydown);
});
</script>

<template>
  <div ref="hostRef" class="table-view-host" @click.stop>
    <button
      class="button ghost compact table-view-button"
      :class="{ active: open || activeId }"
      :disabled="disabled"
      :title="
        disabled
          ? '表数据加载后可保存 / 应用视图'
          : scopeLabel
            ? `表视图 · ${scopeLabel}`
            : '表视图'
      "
      :aria-expanded="open"
      aria-haspopup="menu"
      @click="toggleMenu"
    >
      <Icon name="view" :size="14" />
      <span class="table-view-label">{{ buttonLabel }}</span>
      <span
        v-if="dirty"
        class="table-view-dirty"
        title="当前设置与已保存视图不同"
        aria-label="已修改"
        >•</span
      >
      <Icon name="chevron-down" :size="10" />
    </button>

    <div
      v-if="open"
      class="table-view-menu"
      role="menu"
      aria-label="表视图"
    >
      <div class="table-view-head">
        <span>{{ manage ? "管理视图" : "视图" }}</span>
        <button
          v-if="profiles.length"
          class="link-button"
          @click="manage = !manage"
        >
          {{ manage ? "完成" : "管理…" }}
        </button>
      </div>

      <div v-if="!profiles.length" class="table-view-empty">
        还没有保存的视图。保存当前的筛选、排序、分页、列宽、隐藏列与冻结列，下次一键恢复。
      </div>

      <template v-else>
        <div
          v-for="profile in profiles"
          :key="profile.id"
          class="table-view-row"
          :class="{ active: profile.id === activeId }"
        >
          <button
            class="context-menu-item table-view-item"
            role="menuitem"
            :title="profile.filter ? `WHERE ${profile.filter}` : '无筛选'"
            @click="applyProfile(profile)"
          >
            <Icon
              v-if="profile.id === activeId"
              name="check"
              :size="13"
            />
            <span v-else class="table-view-check-slot"></span>
            <span class="table-view-name">{{ profile.name }}</span>
            <span
              v-if="profile.id === defaultId"
              class="table-view-default"
              title="打开表时自动应用"
              >默认</span
            >
            <span v-if="dirty && profile.id === activeId" class="table-view-dirty">
              已修改
            </span>
          </button>
          <template v-if="manage">
            <button
              class="icon-button"
              title="重命名"
              aria-label="重命名视图"
              @click="startRename(profile)"
            >
              <Icon name="edit" :size="13" />
            </button>
            <button
              class="icon-button"
              :class="{ active: profile.id === defaultId }"
              :title="
                profile.id === defaultId ? '取消默认视图' : '设为默认视图'
              "
              :aria-pressed="profile.id === defaultId"
              @click="toggleDefault(profile)"
            >
              <Icon name="pin" :size="13" />
            </button>
            <button
              class="icon-button danger"
              title="删除视图"
              aria-label="删除视图"
              @click="requestDelete(profile)"
            >
              <Icon name="trash" :size="13" />
            </button>
          </template>
        </div>
      </template>

      <p v-if="error" class="table-view-error">{{ error }}</p>
      <div class="context-separator"></div>
      <button
        v-if="activeProfile && dirty"
        class="context-menu-item"
        role="menuitem"
        @click="updateCurrent"
      >
        <Icon name="refresh" :size="13" />
        更新当前视图
      </button>
      <button
        class="context-menu-item"
        role="menuitem"
        :disabled="limitReached"
        :title="
          limitReached
            ? `每张表最多 ${MAX_PROFILES_PER_TABLE} 个视图`
            : '保存当前筛选、排序、分页与列布局'
        "
        @click="startSave"
      >
        <Icon name="plus" :size="13" />
        保存当前视图…
      </button>
    </div>

    <div v-if="prompt" class="modal-backdrop" @click.self="cancelPrompt">
      <div
        class="modal prompt-modal table-view-prompt"
        role="dialog"
        aria-modal="true"
        :aria-label="prompt.kind === 'save' ? '保存表视图' : '重命名表视图'"
      >
        <h2>{{ prompt.kind === "save" ? "保存当前视图" : "重命名视图" }}</h2>
        <label class="import-field">
          名称
          <input
            ref="nameInput"
            v-model="prompt.value"
            :maxlength="MAX_PROFILE_NAME_LENGTH"
            placeholder="例如：已支付订单"
            @keydown.enter.prevent="submitPrompt"
            @keydown.esc.prevent="cancelPrompt"
          />
        </label>
        <label v-if="prompt.kind === 'save'" class="import-check">
          <input v-model="prompt.makeDefault" type="checkbox" />
          设为默认视图（打开表时自动应用）
        </label>
        <p v-if="error" class="table-view-error">{{ error }}</p>
        <p class="hint">
          保存筛选、排序、每页行数、列宽、隐藏列与冻结列；不保存数据或密码。
        </p>
        <footer class="modal-actions">
          <button class="button ghost" @click="cancelPrompt">取消</button>
          <button class="button primary" @click="submitPrompt">
            {{ prompt.kind === "save" ? "保存" : "重命名" }}
          </button>
        </footer>
      </div>
    </div>

    <ConfirmDialog
      :open="Boolean(deleteTarget)"
      title="删除表视图"
      :message="`确定删除「${deleteTarget?.name ?? ''}」吗？`"
      detail="只删除本地保存的视图配置，不影响表数据、筛选或数据库。"
      confirm-text="删除"
      :danger="true"
      @confirm="confirmDelete"
      @cancel="deleteTarget = null"
    />
  </div>
</template>
