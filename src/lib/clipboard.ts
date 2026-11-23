import { isTauri } from "@tauri-apps/api/core";
import {
  readText as nativeReadText,
  writeText as nativeWriteText,
} from "@tauri-apps/plugin-clipboard-manager";

/**
 * WKWebView 里 navigator.clipboard 对非用户手势的写入经常直接拒绝，
 * 对象菜单取完 DDL 再复制时 await 之后手势已经失效，
 * 所以桌面端优先走 Tauri 原生剪贴板（NSPasteboard），浏览器环境再降级。
 */
function execCommandCopy(text: string): boolean {
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.top = "-1000px";
  area.style.left = "-1000px";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.select();
  area.setSelectionRange(0, text.length);
  let copied = false;
  try {
    copied = document.execCommand("copy");
  } catch {
    copied = false;
  } finally {
    area.remove();
  }
  return copied;
}

export async function writeClipboard(text: string): Promise<void> {
  if (isTauri()) {
    try {
      await nativeWriteText(text);
      return;
    } catch {
      // 插件不可用时继续尝试 Web API
    }
  }

  if (navigator.clipboard?.writeText) {
    try {
      await navigator.clipboard.writeText(text);
      return;
    } catch {
      // 继续降级到 execCommand
    }
  }

  if (!execCommandCopy(text)) {
    throw new Error("当前环境不允许写入剪贴板");
  }
}

export async function readClipboard(): Promise<string> {
  if (isTauri()) {
    try {
      return await nativeReadText();
    } catch {
      // 插件不可用时继续尝试 Web API
    }
  }

  if (navigator.clipboard?.readText) {
    return navigator.clipboard.readText();
  }

  throw new Error("当前环境不允许读取剪贴板");
}