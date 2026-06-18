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
