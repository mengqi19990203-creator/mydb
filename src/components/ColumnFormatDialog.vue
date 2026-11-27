<script setup lang="ts">
import { computed, onBeforeUnmount, reactive, ref, watch } from "vue";
import Icon from "./Icon.vue";
import {
  formatDisplayValue,
  formatRuleSummary,
  validateRule,
  type ColumnFormatRule,
  type ColumnFormatType,
  type DateFormatSource,
  type DateFormatTimezone,
  type JsonPathOutput,
  type MaskMode,
} from "../lib/column-format";

const props = defineProps<{
  open: boolean;
  columnName: string;
  columnType?: string | null;
  samples: unknown[];
  initial?: ColumnFormatRule | null;
}>();

const emit = defineEmits<{
  cancel: [];
  apply: [rule: ColumnFormatRule | null];
}>();

const form = reactive({
  type: "date" as ColumnFormatType,
  dateSource: "auto" as DateFormatSource,
  datePattern: "YYYY-MM-DD HH:mm:ss",
  dateTimezone: "local" as DateFormatTimezone,
  decimals: 2,
  thousands: true,
  percent: false,
  prefix: "",
  suffix: "",
  path: "$",
  pathOutput: "scalar" as JsonPathOutput,
  maskMode: "all" as MaskMode,
  keepStart: 3,
  keepEnd: 4,
  maskChar: "•" as "•" | "*",
  template: "{value}",
});

const localError = ref("");

function load(rule: ColumnFormatRule | null | undefined) {
  form.type = rule?.type ?? "date";
  form.dateSource = rule?.date?.source ?? "auto";
  form.datePattern = rule?.date?.pattern ?? "YYYY-MM-DD HH:mm:ss";
  form.dateTimezone = rule?.date?.timezone ?? "local";
  form.decimals = rule?.number?.decimals ?? 2;
  form.thousands = rule?.number?.thousands ?? true;
  form.percent = rule?.number?.percent ?? false;
  form.prefix = rule?.number?.prefix ?? "";
  form.suffix = rule?.number?.suffix ?? "";
  form.path = rule?.jsonPath?.path ?? "$";
  form.pathOutput = rule?.jsonPath?.output ?? "scalar";
  form.maskMode = rule?.mask?.mode ?? "all";
  form.keepStart = rule?.mask?.keepStart ?? 3;
  form.keepEnd = rule?.mask?.keepEnd ?? 4;
  form.maskChar = rule?.mask?.maskChar ?? "•";
  form.template = rule?.template?.template ?? "{value}";
  localError.value = "";
}

const rule = computed<ColumnFormatRule>(() => {
  if (form.type === "date") {
    return {
      type: "date",
      date: {
        source: form.dateSource,
        pattern: form.datePattern,
        timezone: form.dateTimezone,
      },
    };
  }
  if (form.type === "number") {
    return {
      type: "number",
      number: {
        decimals: Number(form.decimals),
        thousands: form.thousands,
        percent: form.percent,
        prefix: form.prefix,
        suffix: form.suffix,
      },
    };
  }
  if (form.type === "jsonPath") {
    return {
      type: "jsonPath",
      jsonPath: { path: form.path, output: form.pathOutput },
    };
  }
  if (form.type === "mask") {
    return {
      type: "mask",
      mask: {
        mode: form.maskMode,
        keepStart: Number(form.keepStart),
        keepEnd: Number(form.keepEnd),
        maskChar: form.maskChar,
      },
    };
  }
  return { type: "template", template: { template: form.template } };
});

const validationError = computed(() => validateRule(rule.value));
const summary = computed(() =>
  validationError.value ? validationError.value : formatRuleSummary(rule.value),
);
const preview = computed(() =>
  props.samples.slice(0, 10).map((value) => {
    const formatted = formatDisplayValue(value, rule.value);
    return {
      raw:
        value === null || value === undefined
          ? "NULL"
          : typeof value === "object"
            ? JSON.stringify(value)
            : String(value),
      text: formatted.text,
      warning: formatted.warning,
    };
  }),
);

const datePresets = [
  "YYYY-MM-DD",
  "YYYY-MM-DD HH:mm:ss",
  "YYYY-MM-DD HH:mm:ss.SSS",
  "HH:mm:ss",
];

function apply() {
  if (validationError.value) {
    localError.value = validationError.value;
    return;
  }
  emit("apply", rule.value);
}

function clear() {
  emit("apply", null);
}

function onKeydown(event: KeyboardEvent) {
  if (props.open && event.key === "Escape") emit("cancel");
}

watch(
  () => props.open,
  (open) => {
    if (open) {
      load(props.initial);
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
    <div class="modal column-format-dialog" role="dialog" aria-modal="true">
      <header class="modal-header">
        <div>
          <h2>显示格式</h2>
          <p>
            <code>{{ columnName }}</code>
            <span v-if="columnType" class="hint"> · {{ columnType }}</span>
          </p>
        </div>
        <button class="icon-button" title="关闭" @click="emit('cancel')">
          <Icon name="close" />
        </button>
      </header>

      <div class="column-format-body">
        <p class="hint">
          只改变网格显示，不修改数据库值；复制、导出、筛选、统计仍使用原始值。脱敏不是数据安全边界。
        </p>
        <div class="column-format-type">
          <button
            v-for="item in [
              ['date', '日期时间'],
              ['number', '数字'],
              ['jsonPath', 'JSON 路径'],
              ['mask', '脱敏'],
              ['template', '模板'],
            ] as const"
            :key="item[0]"
            class="segment"
            :class="{ active: form.type === item[0] }"
            @click="form.type = item[0]"
          >
            {{ item[1] }}
          </button>
        </div>

        <div v-if="form.type === 'date'" class="column-format-fields">
          <label>
            日期来源
            <select v-model="form.dateSource">
              <option value="auto">自动（ISO / Extended JSON）</option>
              <option value="iso">ISO 8601 字符串</option>
              <option value="epochSeconds">epoch 秒</option>
              <option value="epochMillis">epoch 毫秒</option>
              <option value="extendedJson">Extended JSON $date</option>
            </select>
          </label>
          <label>
            格式
            <input v-model="form.datePattern" placeholder="YYYY-MM-DD HH:mm:ss" />
          </label>
          <div class="column-format-presets">
            <button
              v-for="preset in datePresets"
              :key="preset"
              class="button ghost compact"
              @click="form.datePattern = preset"
            >
              {{ preset }}
            </button>
          </div>
          <label>
            时区
            <select v-model="form.dateTimezone">
              <option value="local">本地</option>
              <option value="utc">UTC</option>
            </select>
          </label>
          <p class="hint">支持 YYYY / MM / DD / HH / mm / ss / SSS；不自动把纯数字 ID 当日期。</p>
        </div>

        <div v-else-if="form.type === 'number'" class="column-format-fields">
          <label>
            小数位
            <input v-model.number="form.decimals" type="number" min="0" max="6" />
          </label>
          <label class="checkbox-row">
            <input v-model="form.thousands" type="checkbox" /> 千分位
          </label>
          <label class="checkbox-row">
            <input v-model="form.percent" type="checkbox" /> 百分比（值 ×100）
          </label>
          <label>
            前缀
            <input v-model="form.prefix" maxlength="20" placeholder="¥" />
          </label>
          <label>
            后缀
            <input v-model="form.suffix" maxlength="20" placeholder="元" />
          </label>
        </div>

        <div v-else-if="form.type === 'jsonPath'" class="column-format-fields">
          <label>
            JSON 路径
            <input v-model="form.path" placeholder="user.name / items[0].id / $.a.b" />
          </label>
          <label>
            输出
            <select v-model="form.pathOutput">
              <option value="scalar">标量文本</option>
              <option value="json">紧凑 JSON</option>
            </select>
          </label>
          <p class="hint">路径最多 200 字符、10 层；不支持通配符、函数或正则。</p>
        </div>

        <div v-else-if="form.type === 'mask'" class="column-format-fields">
          <label>
            脱敏模式
            <select v-model="form.maskMode">
              <option value="all">全部掩码</option>
              <option value="keep">保留前 N 后 M</option>
              <option value="email">邮箱局部</option>
            </select>
          </label>
          <template v-if="form.maskMode === 'keep'">
            <label>
              保留前 N 位
              <input v-model.number="form.keepStart" type="number" min="0" max="20" />
            </label>
            <label>
              保留后 M 位
              <input v-model.number="form.keepEnd" type="number" min="0" max="20" />
            </label>
          </template>
          <label>
            掩码字符
            <select v-model="form.maskChar">
              <option value="•">•</option>
              <option value="*">*</option>
            </select>
          </label>
          <p class="hint">只影响显示；复制单元格、整行 JSON、导出和值查看器仍能看到原文。</p>
        </div>

        <div v-else class="column-format-fields">
          <label>
            模板
            <input v-model="form.template" maxlength="200" placeholder="{value} 元" />
          </label>
          <p class="hint">只支持 {value} 替换，不执行表达式，也不能引用其他列。</p>
        </div>

        <p v-if="localError || validationError" class="data-message warning">
          {{ localError || validationError }}
        </p>

        <section class="column-format-preview">
          <h3>预览 <span class="hint">· {{ summary }}</span></h3>
          <p v-if="!preview.length" class="hint">暂无样本，仍可保存规则。</p>
          <table v-else>
            <thead><tr><th>原始值</th><th>显示值</th></tr></thead>
            <tbody>
              <tr v-for="(item, index) in preview" :key="index">
                <td :title="item.raw">{{ item.raw }}</td>
                <td :class="{ 'format-cell-warning': item.warning }" :title="item.warning ?? undefined">
                  {{ item.text }}
                  <span v-if="item.warning" class="format-warning-dot" aria-label="格式警告">!</span>
                </td>
              </tr>
            </tbody>
          </table>
        </section>
      </div>

      <footer class="modal-actions">
        <button
          v-if="initial"
          class="button ghost column-format-clear"
          @click="clear"
        >
          清除此列格式
        </button>
        <button class="button ghost" @click="emit('cancel')">取消</button>
        <button class="button primary" :disabled="Boolean(validationError)" @click="apply">
          应用
        </button>
      </footer>
    </div>
  </div>
</template>
