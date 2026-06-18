export interface FileNode {
  path: string
  name: string
  is_dir: boolean
  children?: FileNode[]
  file_type: string
}

export interface VersionInfo {
  id: string
  file_path: string
  version_number: number
  timestamp: string
  diff_patch: string
  size: number
  message?: string
}

export interface SearchResult {
  file_path: string
  version_number?: number
  score: number
  snippet: string
  timestamp: string
  file_type: string
}

export interface SearchFilters {
  date_from?: string
  date_to?: string
  file_types?: string[]
  keywords?: string[]
}

export interface TimelineGroup {
  date: string
  items: TimelineItem[]
}

export interface TimelineItem {
  id: string
  file_path: string
  file_name: string
  version_number: number
  timestamp: string
  message?: string
}

export interface SyncStats {
  files_synced: number
  blocks_transferred: number
  bytes_transferred: number
  conflicts_detected: number
  conflicts_resolved: number
  last_sync_time: string | null
  connected_clients: number
  is_server_mode: boolean
}

export interface ConflictInfo {
  file_path: string
  local_node: RemoteVersionNode
  remote_node: RemoteVersionNode
  resolved_winner: string
  resolution_strategy: string
  needs_manual_merge: boolean
}

export interface RemoteVersionNode {
  node_id: string
  client_id: string
  version_number: number
  content_hash: string
  timestamp: string
  file_size: number
  parent_hash: string | null
  block_hashes: string[]
}

export interface PeerInfo {
  client_id: string
  hostname: string
  address: string
  listen_port: number
  version_count: number
}

export interface SyncResult {
  versions_pulled: number
  blocks_pulled: number
  bytes_pulled: number
  conflicts_found: number
}
