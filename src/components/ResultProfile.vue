<script setup lang="ts">
import {
  computed,
  nextTick,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
} from "vue";
import Icon from "./Icon.vue";
import { writeClipboard } from "../lib/clipboard";
import type { ChartData } from "../lib/result-chart";
import {
  PROFILE_COLUMN_LIMIT,
  PROFILE_TYPE_LABELS,
  profileResult,
  profileToMarkdown,
  profileToTsv,
  type ColumnProfile,
  type ProfileValue,
} from "../lib/result-profile";

const props = withDefaults(
  defineProps<{
    data: ChartData;
    dialog?: boolean;
    filterCount?: number;
    filterSummary?: string | null;
    pinnedName?: string | null;
    tablePage?: {
      page: number;
      pageCount: number;
      total: number;
      pending?: boolean;
    } | null;
  }>(),
  { dialog: false, filterCount: 0 },
);

const emit = defineEmits<{
  close: [];
}>();

type SortKey = "display" | "name" | "nulls" | "distinct" | "type";

const displayLimit = ref(PROFILE_COLUMN_LIMIT);
const sortKey = ref<SortKey>("display");
const sortDirection = ref<"asc" | "desc">("asc");
const expanded = ref<Set<string>>(new Set());
const toast = ref("");
const closeButton = ref<HTMLButtonElement | null>(null);
let toastTimer = 0;
let previousFocus: HTMLElement | null = null;

const QUALITY_INFO: Record<string, string> = {
  "全 NULL": "所有已加载行都是 NULL，列里没有可用值。",
  "高 NULL": "NULL 占已加载行超过 30%，使用统计或画图前应确认缺失原因。",
  常量: "所有非 NULL 的值都相同，Distinct 为 1，通常不适合作为图表维度。",
  含空白串: "存在空串或只包含空白字符的字符串；空串与 NULL 分开统计。",
  高重复: "超过一半的非空行属于重复值，Distinct 远小于非空行数。",
  混合类型: "同一列出现数值、文本、布尔或对象等不同类型，数值统计只计算数值项。",
  文本数字: "数字以字符串形式出现；数值统计会包含这些值，但原始类型仍是文本。",
  首尾空白: "字符串存在前导或尾随空格，比较和去重前可能需要确认是否清洗。",
};

const sourcePrimary = computed(() => {
  const columns = props.data.visibleColumns.length;
  const rows = props.data.rows.length;
  if (props.tablePage) {
    return `当前页 ${rows} 行（非全表）· 第 ${props.tablePage.page} / ${props.tablePage.pageCount} 页 · 全表共 ${props.tablePage.total} 行 · ${columns} 列`;
  }
  if (props.pinnedName) {
    return `固定结果 · ${props.pinnedName} · ${rows} 行 · ${columns} 列`;
  }
  return `当前结果 ${rows} 行 · 筛选 ${props.filterCount} 个条件 · ${columns} 列`;
});

const sourceHint = computed(() => {
  if (props.tablePage) {
    return "只剖析当前页数据库行，不代表全表；需要全表统计请先导出或使用 SQL 聚合。";
  }
  if (props.pinnedName) {
    return "剖析固定结果的快照；包含固定视图自己的筛选 / 排序，不包含实时网格的筛选 / 排序。";
  }
  return "剖析当前结果已加载的行，包含本地筛选 / 排序后的可见列。";
});

const sourceTitle = computed(() => {
  if (props.tablePage) {
    return `${props.tablePage.page ? `第 ${props.tablePage.page} 页` : "当前页"} · ${props.data.rows.length} 行`;
  }
  if (props.pinnedName) return `固定结果 · ${props.pinnedName}`;
  return `当前结果 · ${props.data.rows.length} 行`;
});

const profileData = computed(() =>
  profileResult(
    {
      columns: props.data.columns,
      visibleColumns: props.data.visibleColumns,
      rows: props.data.rows,
      truncated: props.data.truncated,
      filterSummary: props.filterSummary ?? null,
      sourceLabel: sourceTitle.value,
    },
    Math.min(displayLimit.value, props.data.visibleColumns.length),
  ),
);

function sortProfiles(profiles: ColumnProfile[]): ColumnProfile[] {
  if (sortKey.value === "display") return profiles;
  const direction = sortDirection.value === "asc" ? 1 : -1;
  return [...profiles].sort((left, right) => {
    let result = 0;
    if (sortKey.value === "name") {
      result = left.name.localeCompare(right.name, "zh-CN", {
        numeric: true,
      });
    } else if (sortKey.value === "nulls") {
      result = left.nullPercent - right.nullPercent;
    } else if (sortKey.value === "distinct") {
      result = left.distinct - right.distinct;
    } else if (sortKey.value === "type") {
      result = PROFILE_TYPE_LABELS[left.type].localeCompare(
        PROFILE_TYPE_LABELS[right.type],
        "zh-CN",
      );
    }
    if (result === 0) result = left.index - right.index;
    return result * direction;
  });
}

const sortedProfiles = computed(() => sortProfiles(profileData.value.columns));
const totalColumnCount = computed(() => props.data.visibleColumns.length);
const hasMoreColumns = computed(
  () => profileData.value.columns.length < totalColumnCount.value,
);
const nextBatchSize = computed(() =>
  Math.min(
    50,
    totalColumnCount.value - profileData.value.columns.length,
  ),
);
const loadedColumnCount = computed(() => profileData.value.columns.length);

watch(sortKey, (key) => {
  sortDirection.value =
    key === "display" || key === "name" ? "asc" : "desc";
});

watch(
  () => profileData.value.columns.map((column) => column.key).join("\u0001"),
  () => {
    const keys = new Set(profileData.value.columns.map((column) => column.key));
    const next = new Set(
      [...expanded.value].filter((key) => keys.has(key)),
    );
    if (!next.size && profileData.value.columns.length) {
      next.add(profileData.value.columns[0].key);
    }
    expanded.value = next;
  },
  { immediate: true },
);

watch(
  () => props.data.visibleColumns.length,
  (total) => {
    if (displayLimit.value > total) displayLimit.value = Math.max(
      PROFILE_COLUMN_LIMIT,
      total,
    );
  },
);

function showMore() {
  displayLimit.value = Math.min(
    totalColumnCount.value,
    displayLimit.value + 50,
  );
}

function toggleColumn(key: string) {
  const next = new Set(expanded.value);
  if (next.has(key)) next.delete(key);
  else next.add(key);
  expanded.value = next;
}

function onRowKeydown(event: KeyboardEvent, key: string) {
  if (event.target !== event.currentTarget) return;
  if (event.key !== "Enter" && event.key !== " ") return;
  event.preventDefault();
  toggleColumn(key);
}

function formatMetric(value: number | null): string {
  if (value === null || !Number.isFinite(value)) return "—";
  return Number(value.toFixed(3)).toLocaleString("zh-CN", {
    maximumFractionDigits: 3,
  });
}

function formatPercent(value: number): string {
  if (!Number.isFinite(value)) return "0%";
  return `${value.toFixed(1)}%`;
}

function typeLabel(profile: ColumnProfile): string {
  return PROFILE_TYPE_LABELS[profile.type];
}

function displayValue(value: string, limit = 28): string {
  if (!value) return "(空串)";
  if (!value.trim()) return "(空白串)";
  return value.length > limit ? `${value.slice(0, limit - 1)}…` : value;
}

function topValues(profile: ColumnProfile): ProfileValue[] {
  return profile.top.filter((value) => !value.blank);
}

function previewTop(profile: ColumnProfile): ProfileValue[] {
  return topValues(profile).slice(0, 3);
}

function topPreviewRemaining(profile: ColumnProfile): number {
  return Math.max(0, topValues(profile).length - 3);
}

function numericCount(profile: ColumnProfile): number {
  return profile.histogram.reduce((total, bin) => total + bin.count, 0);
}

function nonNumericCount(profile: ColumnProfile): number {
  return Math.max(0, profile.total - profile.nulls - numericCount(profile));
}

function categoryMax(profile: ColumnProfile): number {
  const counts = [
    profile.nulls,
    profile.blanks,
    ...topValues(profile).map((value) => value.count),
    profile.otherCount,
  ];
  return Math.max(1, ...counts);
}

function categoryBar(profile: ColumnProfile, count: number): string {
  return `${Math.max(count > 0 ? 2 : 0, (count / categoryMax(profile)) * 100)}%`;
}

function histogramMax(profile: ColumnProfile): number {
  return Math.max(1, ...profile.histogram.map((bin) => bin.count));
}

function histogramBar(profile: ColumnProfile, count: number): string {
  return `${Math.max(count > 0 ? 2 : 0, (count / histogramMax(profile)) * 100)}%`;
}

function lengthMax(profile: ColumnProfile): number {
  return Math.max(1, ...profile.lengthHistogram.map((bin) => bin.count));
}

function lengthBar(profile: ColumnProfile, count: number): string {
  return `${Math.max(count > 0 ? 2 : 0, (count / lengthMax(profile)) * 100)}%`;
}

function hasNumeric(profile: ColumnProfile): boolean {
  return numericCount(profile) > 0;
}

function hasLength(profile: ColumnProfile): boolean {
  return profile.minLength !== null && profile.maxLength !== null;
}

function ariaTop(profile: ColumnProfile, value: ProfileValue): string {
  return `${profile.key} 的值 ${value.label || "(空白串)"} 出现 ${value.count} 次，占 ${formatPercent(value.percent)}`;
}

function ariaBin(label: string, count: number, total: number): string {
  return `${label}：${count} 个值，占 ${formatPercent(total ? (count / total) * 100 : 0)}`;
}

function qualityInfo(label: string): string {
  return QUALITY_INFO[label] ?? "该列存在需要关注的数据质量特征。";
}

function openQuality(profile: ColumnProfile) {
  if (!expanded.value.has(profile.key)) toggleColumn(profile.key);
}

function showToast(message: string) {
  toast.value = message;
  window.clearTimeout(toastTimer);
  toastTimer = window.setTimeout(() => {
    toast.value = "";
  }, 2200);
}

async function copyText(text: string, label: string) {
  try {
    await writeClipboard(text);
    showToast(label);
  } catch (reason) {
    showToast(`复制失败：${reason}`);
  }
}

function copyColumn(profile: ColumnProfile, format: "tsv" | "markdown") {
  if (format === "markdown") {
    void copyText(
      profileToMarkdown([profile], sourceTitle.value),
      `已复制「${profile.key}」Markdown 统计`,
    );
    return;
  }
  void copyText(
    profileToTsv([profile], sourceTitle.value),
    `已复制「${profile.key}」TSV 统计`,
  );
}

function copyAll() {
  const all = profileResult(
    {
      columns: props.data.columns,
      visibleColumns: props.data.visibleColumns,
      rows: props.data.rows,
      truncated: props.data.truncated,
      filterSummary: props.filterSummary ?? null,
      sourceLabel: sourceTitle.value,
    },
    props.data.visibleColumns.length,
  );
  const profiles = sortProfiles(all.columns);
  void copyText(
    profileToTsv(profiles, sourceTitle.value),
    `已复制 ${profiles.length} 列统计（TSV）`,
  );
}

function onWindowKeydown(event: KeyboardEvent) {
  if (!props.dialog) return;
  if (event.key === "Escape") {
    event.preventDefault();
    emit("close");
  }
}

onMounted(() => {
  if (!props.dialog) return;
  previousFocus =
    document.activeElement instanceof HTMLElement
      ? document.activeElement
      : null;
  window.addEventListener("keydown", onWindowKeydown);
  void nextTick(() => closeButton.value?.focus());
});

onBeforeUnmount(() => {
  window.clearTimeout(toastTimer);
  if (props.dialog) {
    window.removeEventListener("keydown", onWindowKeydown);
    if (previousFocus?.isConnected) {
      void nextTick(() => previousFocus?.focus());
    }
  }
});
</script>

<template>
  <Teleport to="body" :disabled="!dialog">
    <div
      :class="
        dialog
          ? 'modal-backdrop result-profile-backdrop'
          : 'result-profile-host'
      "
      @click.self="dialog && emit('close')"
    >
      <section
        class="result-profile"
        :class="{ 'result-profile-dialog': dialog }"
        :role="dialog ? 'dialog' : 'region'"
        :aria-modal="dialog ? 'true' : undefined"
        aria-label="列剖析"
      >
        <header class="profile-head">
          <div class="profile-title">
            <h2>列剖析</h2>
            <p class="hint">
              推断类型基于已加载值，不重新查询数据库；NULL 与空串分开统计。
            </p>
          </div>
          <div class="profile-actions">
            <label class="profile-sort">
              排序
              <select v-model="sortKey" aria-label="排序剖析列">
                <option value="display">显示顺序</option>
                <option value="name">列名</option>
                <option value="nulls">NULL 比例</option>
                <option value="distinct">Distinct</option>
                <option value="type">类型</option>
              </select>
            </label>
            <button
              class="icon-button"
              :title="sortDirection === 'asc' ? '升序（点击降序）' : '降序（点击升序）'"
              :aria-label="sortDirection === 'asc' ? '切换为降序' : '切换为升序'"
              :disabled="sortKey === 'display'"
              @click="
                sortDirection = sortDirection === 'asc' ? 'desc' : 'asc'
              "
            >
              <Icon
                :name="sortDirection === 'asc' ? 'chevron-up' : 'chevron-down'"
                :size="13"
              />
            </button>
            <button class="button ghost compact" @click="copyAll">
              <Icon name="copy" :size="13" />
              复制全部统计
            </button>
            <button
              v-if="dialog"
              ref="closeButton"
              class="icon-button profile-close"
              title="关闭"
              aria-label="关闭列剖析"
              @click="emit('close')"
            >
              <Icon name="close" :size="14" />
            </button>
          </div>
        </header>

        <div class="profile-body">
          <div class="profile-source">
            <Icon name="list" :size="14" />
            <span class="profile-source-text">{{ sourcePrimary }}</span>
            <span v-if="data.truncated" class="profile-chip warning">
              结果已截断，仅剖析已加载行
            </span>
            <span v-if="tablePage?.pending" class="profile-chip warning">
              不含待提交修改
            </span>
          </div>
          <p class="profile-scope-note">{{ sourceHint }}</p>
          <p v-if="filterSummary" class="profile-scope-note">
            当前筛选：<code>{{ filterSummary }}</code>
          </p>

          <p
            v-if="hasMoreColumns"
            class="data-message warning profile-limit"
          >
            当前可见 {{ totalColumnCount }} 列，已剖析前
            {{ loadedColumnCount }} 列。
            <button class="link-button" @click="showMore">
              再显示 {{ nextBatchSize }} 列
            </button>
          </p>

          <div v-if="!data.rows.length" class="panel-empty profile-empty">
            <Icon name="list" :size="18" />
            <span>没有可剖析的行</span>
          </div>
          <div
            v-else-if="!totalColumnCount"
            class="panel-empty profile-empty"
          >
            <Icon name="list" :size="18" />
            <span>没有可见列；请先显示至少一列。</span>
          </div>

          <div v-else class="profile-table-wrap">
            <table class="profile-table">
              <thead>
                <tr>
                  <th class="profile-col-name">列</th>
                  <th>类型</th>
                  <th>NULL</th>
                  <th>空白串</th>
                  <th>Distinct</th>
                  <th>唯一</th>
                  <th>重复</th>
                  <th>统计 / 范围</th>
                  <th>Top 值</th>
                  <th>
                    <span class="sr-only">操作</span>
                  </th>
                </tr>
              </thead>
              <tbody>
                <template
                  v-for="profile in sortedProfiles"
                  :key="profile.key"
                >
                  <tr
                    class="profile-row"
                    :class="{ expanded: expanded.has(profile.key) }"
                    tabindex="0"
                    :aria-expanded="expanded.has(profile.key)"
                    @click="toggleColumn(profile.key)"
                    @keydown="onRowKeydown($event, profile.key)"
                    @contextmenu.prevent="copyColumn(profile, 'markdown')"
                  >
                    <td class="profile-col-name">
                      <div class="profile-name-line">
                        <button
                          class="profile-expand"
                          :aria-expanded="expanded.has(profile.key)"
                          :aria-label="`${expanded.has(profile.key) ? '收起' : '展开'} ${profile.name} 详情`"
                          @click.stop="toggleColumn(profile.key)"
                        >
                          <Icon
                            :name="
                              expanded.has(profile.key)
                                ? 'chevron-down'
                                : 'chevron-right'
                            "
                            :size="12"
                          />
                        </button>
                        <span
                          class="profile-name"
                          :title="
                            profile.key !== profile.name
                              ? `列名重复：${profile.key}`
                              : profile.name
                          "
                        >
                          {{ profile.name }}
                        </span>
                        <span
                          v-if="profile.key !== profile.name"
                          class="profile-dup"
                          :title="`重复列名 ${profile.name} 的稳定键 ${profile.key}`"
                        >
                          #{{ profile.key.split('#').pop() }}
                        </span>
                      </div>
                      <div
                        v-if="profile.quality.length"
                        class="profile-quality"
                      >
                        <button
                          v-for="label in profile.quality"
                          :key="label"
                          class="quality-chip"
                          :title="qualityInfo(label)"
                          @click.stop="openQuality(profile)"
                        >
                          {{ label }}
                        </button>
                      </div>
                    </td>
                    <td>
                      <span class="profile-type" :class="profile.type">
                        {{ typeLabel(profile) }}
                      </span>
                    </td>
                    <td>
                      <span class="profile-number">{{ profile.nulls }}</span>
                      <span class="profile-pct">
                        {{ formatPercent(profile.nullPercent) }}
                      </span>
                    </td>
                    <td>
                      <span class="profile-number">{{ profile.blanks }}</span>
                      <span class="profile-pct">
                        {{ formatPercent(profile.blankPercent) }}
                      </span>
                    </td>
                    <td>
                      <span class="profile-number">{{ profile.distinct }}</span>
                    </td>
                    <td>
                      <span class="profile-number">{{ profile.unique }}</span>
                    </td>
                    <td>
                      <span class="profile-number">
                        {{ profile.duplicateValues }}
                      </span>
                      <span class="profile-pct">
                        {{ formatPercent(profile.duplicatePercent) }}
                      </span>
                    </td>
                    <td class="profile-metrics-cell">
                      <div v-if="hasNumeric(profile)" class="profile-metrics">
                        <span>
                          min {{ formatMetric(profile.min) }} · max
                          {{ formatMetric(profile.max) }}
                        </span>
                        <span>
                          平均 {{ formatMetric(profile.average) }} · 标准差
                          {{ formatMetric(profile.stddev) }}
                        </span>
                      </div>
                      <div
                        v-else-if="profile.type === 'date-like'"
                        class="profile-metrics"
                      >
                        <span>{{ profile.dateMin }}</span>
                        <span>→ {{ profile.dateMax }}</span>
                      </div>
                      <div v-else-if="hasLength(profile)" class="profile-metrics">
                        <span>
                          长度 {{ profile.minLength }} – {{ profile.maxLength }}
                        </span>
                        <span>平均 {{ formatMetric(profile.averageLength) }}</span>
                      </div>
                      <span v-else class="hint">—</span>
                    </td>
                    <td class="profile-top-cell">
                      <div v-if="previewTop(profile).length" class="profile-top">
                        <span
                          v-for="value in previewTop(profile)"
                          :key="value.label"
                          class="profile-top-item"
                          :title="ariaTop(profile, value)"
                        >
                          {{ displayValue(value.label, 18) }}
                          <em>
                            {{ value.count }} / {{ formatPercent(value.percent) }}
                          </em>
                        </span>
                        <span
                          v-if="topPreviewRemaining(profile)"
                          class="profile-top-more"
                        >
                          +{{ topPreviewRemaining(profile) }}
                        </span>
                      </div>
                      <span v-else-if="profile.blanks" class="hint">
                        空白串 {{ profile.blanks }}
                      </span>
                      <span v-else class="hint">—</span>
                    </td>
                    <td class="profile-row-actions">
                      <button
                        class="icon-button"
                        title="复制列统计 TSV（右键复制 Markdown）"
                        :aria-label="`复制 ${profile.name} 的统计`"
                        @click.stop="copyColumn(profile, 'tsv')"
                      >
                        <Icon name="copy" :size="13" />
                      </button>
                    </td>
                  </tr>
                  <tr
                    v-if="expanded.has(profile.key)"
                    class="profile-detail-row"
                  >
                    <td :colspan="10">
                      <div class="profile-detail-grid">
                        <section class="profile-detail-card">
                          <h4>质量与示例</h4>
                          <p class="profile-detail-note">
                            推断类型：<strong>{{ typeLabel(profile) }}</strong>
                            · 统计按显示值；NULL 不计入 Distinct / 唯一 / Top。
                          </p>
                          <ul
                            v-if="profile.quality.length"
                            class="profile-quality-list"
                          >
                            <li
                              v-for="label in profile.quality"
                              :key="label"
                            >
                              <strong>{{ label }}</strong>
                              {{ qualityInfo(label) }}
                            </li>
                          </ul>
                          <p v-else class="hint">未发现明显质量问题。</p>
                          <div
                            v-if="profile.examples.length"
                            class="profile-examples"
                          >
                            <span>示例值</span>
                            <code
                              v-for="example in profile.examples"
                              :key="example"
                              :title="example"
                            >
                              {{ displayValue(example, 80) }}
                            </code>
                          </div>
                        </section>

                        <section class="profile-detail-card">
                          <h4>
                            分类分布
                            <span class="hint">Top 20</span>
                          </h4>
                          <p
                            v-if="
                              profile.type === 'empty' &&
                              !profile.nulls &&
                              !topValues(profile).length
                            "
                            class="hint"
                          >
                            没有可显示的分类值。
                          </p>
                          <template v-else>
                            <div
                              v-if="profile.nulls"
                              class="profile-dist-row"
                            >
                              <span class="profile-dist-label">NULL</span>
                              <div
                                class="profile-bar-track"
                                role="img"
                                :aria-label="`NULL 出现 ${profile.nulls} 次，占 ${formatPercent(profile.nullPercent)}`"
                              >
                                <div
                                  class="profile-bar null"
                                  :style="{ width: categoryBar(profile, profile.nulls) }"
                                ></div>
                              </div>
                              <span class="profile-dist-count">
                                {{ profile.nulls }}
                                <em>{{ formatPercent(profile.nullPercent) }}</em>
                              </span>
                            </div>
                            <div
                              v-if="profile.blanks"
                              class="profile-dist-row"
                            >
                              <span class="profile-dist-label">空白串</span>
                              <div
                                class="profile-bar-track"
                                role="img"
                                :aria-label="`空白串出现 ${profile.blanks} 次，占 ${formatPercent(profile.blankPercent)}`"
                              >
                                <div
                                  class="profile-bar blank"
                                  :style="{ width: categoryBar(profile, profile.blanks) }"
                                ></div>
                              </div>
                              <span class="profile-dist-count">
                                {{ profile.blanks }}
                                <em>{{ formatPercent(profile.blankPercent) }}</em>
                              </span>
                            </div>
                            <div
                              v-for="value in topValues(profile)"
                              :key="value.label"
                              class="profile-dist-row"
                            >
                              <span
                                class="profile-dist-label"
                                :title="value.label"
                              >
                                {{ displayValue(value.label, 40) }}
                              </span>
                              <div
                                class="profile-bar-track"
                                role="img"
                                :aria-label="ariaTop(profile, value)"
                              >
                                <div
                                  class="profile-bar"
                                  :style="{ width: categoryBar(profile, value.count) }"
                                ></div>
                              </div>
                              <span class="profile-dist-count">
                                {{ value.count }}
                                <em>{{ formatPercent(value.percent) }}</em>
                              </span>
                            </div>
                            <div
                              v-if="profile.otherValues"
                              class="profile-dist-row other"
                            >
                              <span class="profile-dist-label">
                                其他（{{ profile.otherValues }} 个值）
                              </span>
                              <div
                                class="profile-bar-track"
                                role="img"
                                :aria-label="`其他 ${profile.otherValues} 个值共出现 ${profile.otherCount} 次`"
                              >
                                <div
                                  class="profile-bar other"
                                  :style="{ width: categoryBar(profile, profile.otherCount) }"
                                ></div>
                              </div>
                              <span class="profile-dist-count">
                                {{ profile.otherCount }}
                                <em>
                                  {{ formatPercent((profile.otherCount / Math.max(1, profile.total)) * 100) }}
                                </em>
                              </span>
                            </div>
                          </template>
                        </section>

                        <section
                          v-if="hasNumeric(profile)"
                          class="profile-detail-card"
                        >
                          <h4>
                            数值分布
                            <span class="hint">10 个等宽区间</span>
                          </h4>
                          <div
                            v-for="bin in profile.histogram"
                            :key="bin.label"
                            class="profile-dist-row"
                          >
                            <span class="profile-dist-label" :title="bin.label">
                              {{ bin.label }}
                            </span>
                            <div
                              class="profile-bar-track"
                              role="img"
                              :aria-label="ariaBin(bin.label, bin.count, numericCount(profile))"
                            >
                              <div
                                class="profile-bar numeric"
                                :style="{ width: histogramBar(profile, bin.count) }"
                              ></div>
                            </div>
                            <span class="profile-dist-count">
                              {{ bin.count }}
                            </span>
                          </div>
                          <p class="hint">
                            共 {{ numericCount(profile) }} 个数值 · 非数值
                            {{ nonNumericCount(profile) }} · NULL
                            {{ profile.nulls }}
                          </p>
                        </section>

                        <section
                          v-if="hasLength(profile)"
                          class="profile-detail-card"
                        >
                          <h4>文本长度</h4>
                          <div class="profile-length-summary">
                            <span>最短 {{ profile.minLength }}</span>
                            <span>最长 {{ profile.maxLength }}</span>
                            <span>平均 {{ formatMetric(profile.averageLength) }}</span>
                          </div>
                          <div
                            v-for="bin in profile.lengthHistogram"
                            :key="bin.label"
                            class="profile-dist-row"
                          >
                            <span class="profile-dist-label" :title="bin.label">
                              {{ bin.label }}
                            </span>
                            <div
                              class="profile-bar-track"
                              role="img"
                              :aria-label="ariaBin(bin.label, bin.count, profile.total - profile.nulls)"
                            >
                              <div
                                class="profile-bar length"
                                :style="{ width: lengthBar(profile, bin.count) }"
                              ></div>
                            </div>
                            <span class="profile-dist-count">
                              {{ bin.count }}
                            </span>
                          </div>
                        </section>

                        <section
                          v-if="profile.dateMin !== null"
                          class="profile-detail-card"
                        >
                          <h4>日期范围</h4>
                          <p class="profile-date-range">
                            <span>{{ profile.dateMin }}</span>
                            <span>→</span>
                            <span>{{ profile.dateMax }}</span>
                          </p>
                          <p class="hint">
                            按字符串排序，不做时区转换；只作展示辅助。
                          </p>
                        </section>
                      </div>
                    </td>
                  </tr>
                </template>
              </tbody>
            </table>
          </div>
        </div>

        <div v-if="toast" class="profile-toast" role="status" aria-live="polite">
          {{ toast }}
        </div>
      </section>
    </div>
  </Teleport>
</template>
