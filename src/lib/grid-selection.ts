export interface GridCell {
  row: number;
  column: number;
}

export interface GridSelection {
  anchor: GridCell;
  focus: GridCell;
}

export interface GridRange {
  top: number;
  bottom: number;
  left: number;
  right: number;
}

export function normalizeRange(selection: GridSelection): GridRange {
  return {
    top: Math.min(selection.anchor.row, selection.focus.row),
    bottom: Math.max(selection.anchor.row, selection.focus.row),
    left: Math.min(selection.anchor.column, selection.focus.column),
    right: Math.max(selection.anchor.column, selection.focus.column),
  };
}

export function clampSelection(
  selection: GridSelection,
  rowCount: number,
): GridSelection | null {
  if (rowCount <= 0) return null;
  const clamp = (row: number) => Math.min(rowCount - 1, Math.max(0, row));
  return {
    anchor: { ...selection.anchor, row: clamp(selection.anchor.row) },
    focus: { ...selection.focus, row: clamp(selection.focus.row) },
  };
}

export function isCellSelected(
  selection: GridSelection | null,
  row: number,
  column: number,
): boolean {
  if (!selection) return false;
  const range = normalizeRange(selection);
  return (
    row >= range.top &&
    row <= range.bottom &&
    column >= range.left &&
    column <= range.right
  );
}

export function selectedVisibleColumns(
  selection: GridSelection | null,
  visibleColumns: readonly number[],
): number[] {
  if (!selection) return [];
  const range = normalizeRange(selection);
  return visibleColumns.filter(
    (column) => column >= range.left && column <= range.right,
  );
}

export function selectionCellCount(
  selection: GridSelection | null,
  rowCount: number,
  visibleColumns: readonly number[],
): number {
  if (!selection || rowCount <= 0) return 0;
  const range = normalizeRange(selection);
  const rows = Math.min(rowCount - 1, range.bottom) - Math.min(rowCount - 1, range.top) + 1;
  if (rows <= 0) return 0;
  return rows * selectedVisibleColumns(selection, visibleColumns).length;
}
