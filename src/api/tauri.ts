import { invoke } from "@tauri-apps/api/core"
import type { FileNode, VersionInfo, SearchResult, SearchFilters } from "@/types"

export async function watchFolder(path: string): Promise<void> {
  await invoke("watch_folder", { path })
}

export async function stopWatching(): Promise<void> {
  await invoke("stop_watching")
}

export async function getFileTree(path: string): Promise<FileNode[]> {
  return await invoke<FileNode[]>("get_file_tree", { path })
}

export async function readFileContent(path: string): Promise<string> {
  return await invoke<string>("read_file_content", { path })
}

export async function getFileVersions(path: string): Promise<VersionInfo[]> {
  return await invoke<VersionInfo[]>("get_file_versions", { path })
}

export async function getFileVersionContent(path: string, version: number): Promise<string> {
  return await invoke<string>("get_file_version_content", { path, version })
}

export async function semanticSearch(query: string, filters?: SearchFilters): Promise<SearchResult[]> {
  return await invoke<SearchResult[]>("semantic_search", { query, filters })
}

export async function restoreVersion(path: string, version: number): Promise<void> {
  await invoke("restore_version", { path, version })
}
