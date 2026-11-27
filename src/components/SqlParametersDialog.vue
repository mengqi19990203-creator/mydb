<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import {
  defaultParameterValue,
  substituteSqlParameters,
  validateParameterValue,
  type SqlParameterDefinition,
  type SqlParameterType,
  type SqlParameterValue,
} from "../lib/sql-parameters";
import type { Driver } from "../lib/types";

const props = defineProps<{
  open: boolean;
  sql: string;
  driver: Driver;
  definitions: SqlParameterDefinition[];
  initialValues?: Record<string, SqlParameterValue>;
  error?: string | null;
}>();

const emit = defineEmits<{
  cancel: [];
  run: [values: Record<string, SqlParameterValue>];
}>();

const values = reactive<Record<string, SqlParameterValue>>({});
const localError = ref("");
const previewOpen = ref(false);
const previewCopyMessage = ref("");

function reset() {
  for (const key of Object.keys(values)) delete values[key];
  for (const definition of props.definitions) {
    values[definition.key] =
      props.initialValues?.[definition.key] ??
      defaultParameterValue(definition);
  }
  localError.value = "";
  previewOpen.value = false;
  previewCopyMessage.value = "";
}

function markTouched(key: string) {
  const value = values[key];
  if (value) value.touched = true;
  previewCopyMessage.value = "";
}

function changeType(key: string, type: SqlParameterType) {
  const current = values[key];
  if (!current) return;
  current.type = type;
  previewCopyMessage.value = "";
  if (type === "null") {
    current.value = "";
    current.touched = true;
    current.rawConfirmed = false;
  } else if (type === "boolean") {
    if (current.value !== "true" && current.value !== "false") {
      current.value = "true";
    }
    current.touched = true;
    current.rawConfirmed = false;
  } else if (type === "raw") {
    current.touched = Boolean(current.value);
    current.rawConfirmed = false;
  } else {
    current.touched = current.touched || Boolean(current.value);
    current.rawConfirmed = false;
  }
}

const fieldErrors = computed<Record<string, string>>(() => {
  const result: Record<string, string> = {};
  for (const definition of props.definitions) {
    const error = validateParameterValue(values[definition.key]);
    if (error) result[definition.key] = error;
  }
  return result;
});

const hasErrors = computed(
  () => Boolean(props.error) || Object.keys(fieldErrors.value).length > 0,
);

const preview = computed(() =>
  substituteSqlParameters(
    props.sql,
    values as Record<string, SqlParameterValue>,
    props.driver,
    true,
  ),
);

async function copyPreview() {
  if (preview.value.errors.length) return;
  try {
    await writeClipboard(preview.value.sql);
    previewCopyMessage.value = "已复制掩码预览";
  } catch (reason) {
    localError.value = String(reason);
  }
}

function submit() {
  localError.value = "";
  if (props.error) return;
  if (Object.keys(fieldErrors.value).length) {
    localError.value = "请先完成所有参数";
    return;
  }
  emit("run", JSON.parse(JSON.stringify(values)) as Record<string, SqlParameterValue>);
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("cancel");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      reset();
      document.addEventListener("keydown", onKeydown, true);
    } else {
      document.removeEventListener("keydown", onKeydown, true);
    }
  },
  { immediate: true },
);

onBeforeUnmount(() => document.removeEventListener("keydown", onKeydown, true));
</script>

<template>
  <div v-if="open" class="modal-backdrop" @click.self="emit('cancel')">
    <div class="modal sql-parameters-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>SQL 参数</h2>
          <p>
            检测到 {{ definitions.length }} 个参数 · 只替换本次执行的 SQL，编辑器内容不变
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('cancel')">
          <Icon name="close" />
        </button>
      </header>

      <div class="sql-parameters-body">
        <p class="hint">
          字符串会按当前驱动转义；原始 SQL 会原样插入，必须确认且替换后仍是单条语句。参数值只保存在当前标签会话，不写入历史。
        </p>
        <div class="sql-parameter-list">
          <div
            v-for="definition in definitions"
            :key="definition.key"
            class="sql-parameter-row"
            :class="{ 'has-error': fieldErrors[definition.key] }"
          >
            <div class="sql-parameter-label">
              <code>{{ definition.key }}</code>
              <span class="hint">
                第 {{ definition.statements.join("、") }} 条语句
                · 出现 {{ definition.occurrences }} 次
              </span>
            </div>
            <div class="sql-parameter-controls">
              <select
                :value="values[definition.key]?.type"
                aria-label="参数类型"
                @change="
                  changeType(
                    definition.key,
                    ($event.target as HTMLSelectElement)
                      .value as SqlParameterType,
                  )
                "
              >
                <option value="string">字符串</option>
                <option value="number">数字</option>
                <option value="boolean">布尔</option>
                <option value="null">NULL</option>
                <option value="raw">原始 SQL</option>
              </select>
              <select
                v-if="values[definition.key]?.type === 'boolean'"
                :value="values[definition.key]?.value"
                aria-label="布尔值"
                @change="
                  values[definition.key].value = (
                    $event.target as HTMLSelectElement
                  ).value;
                  markTouched(definition.key);
                "
              >
                <option value="true">TRUE</option>
                <option value="false">FALSE</option>
              </select>
              <span
                v-else-if="values[definition.key]?.type === 'null'"
                class="sql-parameter-null"
              >NULL</span>
              <textarea
                v-else
                :value="values[definition.key]?.value"
                :class="{ mono: values[definition.key]?.type === 'raw' }"
                :placeholder="
                  values[definition.key]?.type === 'raw'
                    ? 'ORDER BY created_at DESC'
                    : '参数值'
                "
                spellcheck="false"
                @input="
                  values[definition.key].value = (
                    $event.target as HTMLTextAreaElement
                  ).value;
                  markTouched(definition.key);
                "
              />
              <label
                v-if="values[definition.key]?.type === 'string'"
                class="checkbox-row sql-parameter-empty"
              >
                <input
                  type="checkbox"
                  :checked="
                    values[definition.key]?.touched &&
                    values[definition.key]?.value === ''
                  "
                  @change="
                    if (($event.target as HTMLInputElement).checked) {
                      values[definition.key].value = '';
                      values[definition.key].touched = true;
                    } else {
                      values[definition.key].touched = false;
                    }
                  "
                />
                明确使用空字符串
              </label>
              <label class="checkbox-row sql-parameter-sensitive">
                <input
                  type="checkbox"
                  :checked="values[definition.key]?.sensitive"
                  @change="
                    values[definition.key].sensitive = (
                      $event.target as HTMLInputElement
                    ).checked
                  "
                />
                敏感
              </label>
            </div>
            <label
              v-if="values[definition.key]?.type === 'raw'"
              class="checkbox-row sql-parameter-raw-confirm"
            >
              <input
                type="checkbox"
                :checked="values[definition.key]?.rawConfirmed"
                @change="
                  values[definition.key].rawConfirmed = (
                    $event.target as HTMLInputElement
                  ).checked
                "
              />
              我确认这是可信 SQL 片段（会原样插入）
            </label>
            <p v-if="fieldErrors[definition.key]" class="data-message warning">
              {{ fieldErrors[definition.key] }}
            </p>
          </div>
        </div>

        <div class="sql-parameter-preview-head">
          <button class="button ghost compact" @click="previewOpen = !previewOpen">
            <Icon name="chevron-down" :size="12" />
            {{ previewOpen ? "收起最终 SQL" : "预览最终 SQL" }}
          </button>
          <span v-if="previewCopyMessage" class="hint">{{ previewCopyMessage }}</span>
          <span class="spacer"></span>
          <button class="button ghost compact" :disabled="preview.errors.length > 0" @click="copyPreview">
            复制预览（掩码）
          </button>
        </div>
        <pre v-if="previewOpen" class="sql-parameter-preview">{{ preview.sql }}</pre>
        <p v-if="previewOpen && preview.errors.length" class="data-message warning">
          {{ preview.errors.join("；") }}
        </p>
        <p v-if="error || localError" class="data-message warning">
          {{ error || localError }}
        </p>
      </div>

      <footer class="modal-actions">
        <button class="button ghost" @click="emit('cancel')">取消</button>
        <button class="button primary" :disabled="hasErrors" @click="submit">
          运行
        </button>
      </footer>
    </div>
  </div>
</template>
