<script setup lang="ts">
import { computed, onBeforeUnmount, ref } from "vue";

const props = defineProps<{
  orientation: "vertical" | "horizontal";
  value: number;
  min: number;
  max: number;
  label: string;
  invert?: boolean;
  mode?: "pixels" | "ratio";
}>();

const emit = defineEmits<{
  "update:value": [value: number];
  reset: [];
}>();

const dragging = ref(false);
let startPosition = 0;
let startValue = 0;
let containerSize = 1;

const ariaValue = computed(() =>
  props.mode === "ratio" ? Math.round(props.value * 100) : Math.round(props.value),
);
const ariaMin = computed(() =>
  props.mode === "ratio" ? Math.round(props.min * 100) : Math.round(props.min),
);
const ariaMax = computed(() =>
  props.mode === "ratio" ? Math.round(props.max * 100) : Math.round(props.max),
);

function clampValue(value: number): number {
  return Math.min(props.max, Math.max(props.min, value));
}

function onPointerDown(event: PointerEvent) {
  if (event.button !== 0) return;
  event.preventDefault();
  const target = event.currentTarget as HTMLElement;
  target.setPointerCapture(event.pointerId);
  dragging.value = true;
  startPosition =
    props.orientation === "vertical" ? event.clientX : event.clientY;
  startValue = props.value;
  containerSize =
    props.mode === "ratio"
      ? target.parentElement?.clientHeight || 1
      : 1;
  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp, { once: true });
  window.addEventListener("pointercancel", onPointerUp, { once: true });
}

function onPointerMove(event: PointerEvent) {
  if (!dragging.value) return;
  const position =
    props.orientation === "vertical" ? event.clientX : event.clientY;
  let delta = position - startPosition;
  if (props.invert) delta = -delta;
  const next =
    props.mode === "ratio" ? startValue + delta / containerSize : startValue + delta;
  emit("update:value", clampValue(next));
}

function onPointerUp() {
  dragging.value = false;
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", onPointerUp);
}

function onKeydown(event: KeyboardEvent) {
  const direction =
    event.key === "ArrowLeft" || event.key === "ArrowUp"
      ? -1
      : event.key === "ArrowRight" || event.key === "ArrowDown"
        ? 1
        : 0;
  if (event.key === "Home" || event.key === "End") {
    event.preventDefault();
    emit(
      "update:value",
      event.key === "Home" ? props.min : props.max,
    );
    return;
  }
  if (!direction) return;
  event.preventDefault();
  const step = props.mode === "ratio" ? 0.02 : 10;
  emit(
    "update:value",
    clampValue(props.value + direction * step * (event.shiftKey ? 3 : 1)),
  );
}

onBeforeUnmount(() => {
  window.removeEventListener("pointermove", onPointerMove);
  window.removeEventListener("pointerup", onPointerUp);
  window.removeEventListener("pointercancel", onPointerUp);
});
</script>

<template>
  <div
    class="resize-handle"
    :class="[orientation, { dragging }]"
    role="separator"
    :aria-label="label"
    :aria-orientation="orientation"
    :aria-valuenow="ariaValue"
    :aria-valuemin="ariaMin"
    :aria-valuemax="ariaMax"
    :title="`${label}（双击恢复默认）`"
    tabindex="0"
    @pointerdown="onPointerDown"
    @dblclick="emit('reset')"
    @keydown="onKeydown"
  ></div>
</template>