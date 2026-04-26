// File: src/services/tauriApi.ts
import { invoke } from "@tauri-apps/api/tauri";
import { open } from "@tauri-apps/api/shell";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import type { SearchResult, IndexStats, AppConfig, IndexProgressEvent } from "../types";

export async function searchFiles(query: string, limit: number = 500): Promise<SearchResult[]> {
  return invoke<SearchResult[]>("search_files", { query, limit });
}

export async function getStats(): Promise<IndexStats> {
  return invoke<IndexStats>("get_stats");
}

export async function rebuildIndex(): Promise<IndexStats> {
  return invoke<IndexStats>("rebuild_index");
}

export async function scanDirectory(path: string): Promise<number> {
  return invoke<number>("scan_directory", { path });
}

export async function loadIndex(): Promise<IndexStats> {
  return invoke<IndexStats>("load_index");
}

export async function indexAllVolumes(): Promise<IndexStats> {
  return invoke<IndexStats>("index_all_volumes");
}

export async function getConfig(): Promise<AppConfig> {
  return invoke<AppConfig>("get_config");
}

export async function saveConfig(config: AppConfig): Promise<void> {
  return invoke<void>("save_config", { config });
}

export async function addDirectory(path: string): Promise<void> {
  return invoke<void>("add_directory", { path });
}

export async function removeDirectory(path: string): Promise<void> {
  return invoke<void>("remove_directory", { path });
}

export async function openFile(path: string): Promise<void> {
  await open(path);
}

export async function openContainingFolder(path: string): Promise<void> {
  const lastSep = Math.max(path.lastIndexOf("/"), path.lastIndexOf("\\"));
  const parent = lastSep > 0 ? path.substring(0, lastSep) : path;
  await open(parent);
}

export async function copyToClipboard(text: string): Promise<void> {
  await navigator.clipboard.writeText(text);
}

/// Listens for index progress events from the Rust backend.
export async function onIndexProgress(
  callback: (event: IndexProgressEvent) => void
): Promise<UnlistenFn> {
  return listen<IndexProgressEvent>("index-progress", (event) => {
    callback(event.payload);
  });
}