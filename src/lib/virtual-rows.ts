import {
  computed,
  onBeforeUnmount,
  onMounted,
  ref,
  watch,
  type Ref,
} from "vue";

interface VirtualRowsOptions {
  /** Scroll container (the element with `overflow: auto`). */
  container: Ref<HTMLElement | null>;
  /** Number of rows currently in the data set. */
  rowCount: Ref<number>;
  /** Fixed row height in pixels. */
  rowHeight?: number;
  /** Extra rows rendered above and below the viewport. */
  overscan?: number;
  /** Force virtualization on / off. Defaults to row count > threshold. */
  enabled?: Ref<boolean>;
  /** Virtualize only when the row count is above this value. */
  threshold?: number;
}

/**
 * Windowed row rendering for large result sets. Returns the half-open slice
 * `[start, end)` plus spacer heights so a plain `<table>` can keep its native
 * sticky header, colgroup widths and menu behaviour.
 */
export function useVirtualRows(options: VirtualRowsOptions) {
  const rowHeight = options.rowHeight ?? 30;
  const overscan = options.overscan ?? 10;
  const threshold = options.threshold ?? 100;
  const scrollTop = ref(0);
  const viewportHeight = ref(0);

  const enabled = computed(
    () =>
      options.enabled?.value ??
      options.rowCount.value > threshold,
  );

  const start = computed(() => {
    if (!enabled.value) return 0;
    return Math.min(
      options.rowCount.value,
      Math.max(0, Math.floor(scrollTop.value / rowHeight) - overscan),
    );
  });

  const end = computed(() => {
    if (!enabled.value) return options.rowCount.value;
    const visible = Math.ceil(viewportHeight.value / rowHeight);
    return Math.max(
      start.value,
      Math.min(
        options.rowCount.value,
        Math.floor(scrollTop.value / rowHeight) + visible + overscan,
      ),
    );
  });

  const paddingTop = computed(() => start.value * rowHeight);
  const paddingBottom = computed(() =>
    Math.max(0, (options.rowCount.value - end.value) * rowHeight),
  );
  const renderedCount = computed(() => Math.max(0, end.value - start.value));

  function measure() {
    const element = options.container.value;
    if (!element) return;
    scrollTop.value = element.scrollTop;
    viewportHeight.value = element.clientHeight;
  }

  let frame = 0;
  function onScroll() {
    if (frame) return;
    frame = window.requestAnimationFrame(() => {
      frame = 0;
      measure();
    });
  }

  let observer: ResizeObserver | null = null;

  onMounted(() => {
    const element = options.container.value;
    if (!element) return;
    measure();
    element.addEventListener("scroll", onScroll, { passive: true });
    if (typeof ResizeObserver !== "undefined") {
      observer = new ResizeObserver(() => measure());
      observer.observe(element);
    }
  });

  onBeforeUnmount(() => {
    if (frame) window.cancelAnimationFrame(frame);
    const element = options.container.value;
    if (element) element.removeEventListener("scroll", onScroll);
    observer?.disconnect();
  });

  watch(
    () => options.rowCount.value,
    () => measure(),
  );

  function scrollToTop() {
    const element = options.container.value;
    if (!element) return;
    element.scrollTop = 0;
    scrollTop.value = 0;
  }

  return {
    enabled,
    start,
    end,
    paddingTop,
    paddingBottom,
    renderedCount,
    scrollToTop,
    /** Exposed for tests / callers that need to force a re-measure. */
    measure,
  };
}