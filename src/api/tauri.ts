import { invoke } from "@tauri-apps/api/core"
import type { FileNode, VersionInfo, SearchResult, SearchFilters, SyncStats, ConflictInfo, PeerInfo, SyncResult } from "@/types"

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

export async function startSyncServer(port: number): Promise<string> {
  return await invoke<string>("start_sync_server", { port })
}

export async function stopSyncServer(): Promise<void> {
  await invoke("stop_sync_server")
}

export async function connectToServer(address: string): Promise<void> {
  await invoke("connect_to_server", { address })
}

export async function disconnectSync(): Promise<void> {
  await invoke("disconnect_sync")
}

export async function publishLocalVersions(): Promise<number> {
  return await invoke<number>("publish_local_versions")
}

export async function pullRemoteVersions(): Promise<SyncResult> {
  return await invoke<SyncResult>("pull_remote_versions")
}

export async function getSyncStats(): Promise<SyncStats> {
  return await invoke<SyncStats>("get_sync_stats")
}

export async function getConflicts(): Promise<ConflictInfo[]> {
  return await invoke<ConflictInfo[]>("get_conflicts")
}

export async function resolveConflict(filePath: string, chooseLocal: boolean): Promise<void> {
  await invoke("resolve_conflict", { filePath, chooseLocal })
}

export async function getConnectedPeers(): Promise<PeerInfo[]> {
  return await invoke<PeerInfo[]>("get_connected_peers")
}
