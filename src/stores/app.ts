import { defineStore } from "pinia"
import { ref } from "vue"
import type { FileNode, VersionInfo, SearchResult, SearchFilters, TimelineGroup, TimelineItem } from "@/types"
import {
  watchFolder,
  stopWatching,
  getFileTree,
  readFileContent,
  getFileVersions,
  getFileVersionContent,
  semanticSearch,
  restoreVersion as apiRestoreVersion,
} from "@/api/tauri"
import { ElMessage } from "element-plus"

export const useAppStore = defineStore("app", () => {
  const watchedFolder = ref<string>("")
  const selectedFile = ref<string>("")
  const selectedFileName = ref<string>("")
  const fileTree = ref<FileNode[]>([])
  const versions = ref<VersionInfo[]>([])
  const searchResults = ref<SearchResult[]>([])
  const isWatching = ref<boolean>(false)
  const fileContent = ref<string>("")
  const isLoading = ref<boolean>(false)
  const activeVersion = ref<number | null>(null)

  async function setWatchedFolder(path: string) {
    try {
      isLoading.value = true
      if (isWatching.value) {
        await stopWatching()
      }
      await watchFolder(path)
      watchedFolder.value = path
      isWatching.value = true
      await refreshTree()
      ElMessage.success(`开始监控文件夹: ${path}`)
    } catch (error) {
      ElMessage.error(`监控文件夹失败: ${error}`)
      isWatching.value = false
    } finally {
      isLoading.value = false
    }
  }

  async function selectFile(path: string, name?: string) {
    selectedFile.value = path
    selectedFileName.value = name || path.split(/[/\\]/).pop() || ""
    activeVersion.value = null
    await Promise.all([loadFileContent(path), loadFileVersions(path)])
  }

  async function loadFileContent(path: string, version?: number) {
    try {
      if (version !== undefined) {
        fileContent.value = await getFileVersionContent(path, version)
        activeVersion.value = version
      } else {
        fileContent.value = await readFileContent(path)
        activeVersion.value = null
      }
    } catch (error) {
      fileContent.value = ""
      ElMessage.error(`读取文件失败: ${error}`)
    }
  }

  async function loadFileVersions(path: string) {
    try {
      versions.value = await getFileVersions(path)
    } catch (error) {
      versions.value = []
      ElMessage.error(`获取版本列表失败: ${error}`)
    }
  }

  async function performSearch(query: string, filters?: SearchFilters) {
    if (!query.trim()) {
      searchResults.value = []
      return
    }
    try {
      isLoading.value = true
      searchResults.value = await semanticSearch(query, filters)
    } catch (error) {
      searchResults.value = []
      ElMessage.error(`搜索失败: ${error}`)
    } finally {
      isLoading.value = false
    }
  }

  async function refreshTree() {
    if (!watchedFolder.value) return
    try {
      isLoading.value = true
      fileTree.value = await getFileTree(watchedFolder.value)
    } catch (error) {
      fileTree.value = []
      ElMessage.error(`刷新文件树失败: ${error}`)
    } finally {
      isLoading.value = false
    }
  }

  async function restoreVersion(path: string, version: number) {
    try {
      await apiRestoreVersion(path, version)
      ElMessage.success(`已恢复到版本 ${version}`)
      await loadFileContent(path)
      await loadFileVersions(path)
    } catch (error) {
      ElMessage.error(`恢复版本失败: ${error}`)
    }
  }

  function getTimelineGroups(): TimelineGroup[] {
    const map = new Map<string, TimelineItem[]>()
    for (const v of versions.value) {
      const date = v.timestamp.split("T")[0]
      const fileName = v.file_path.split(/[/\\]/).pop() || ""
      const item: TimelineItem = {
        id: v.id,
        file_path: v.file_path,
        file_name: fileName,
        version_number: v.version_number,
        timestamp: v.timestamp,
        message: v.message,
      }
      if (!map.has(date)) {
        map.set(date, [])
      }
      map.get(date)!.push(item)
    }
    const groups: TimelineGroup[] = []
    for (const [date, items] of map.entries()) {
      groups.push({
        date,
        items: items.sort((a, b) => (a.timestamp < b.timestamp ? 1 : -1)),
      })
    }
    return groups.sort((a, b) => (a.date < b.date ? 1 : -1))
  }

  return {
    watchedFolder,
    selectedFile,
    selectedFileName,
    fileTree,
    versions,
    searchResults,
    isWatching,
    fileContent,
    isLoading,
    activeVersion,
    setWatchedFolder,
    selectFile,
    loadFileContent,
    loadFileVersions,
    performSearch,
    refreshTree,
    restoreVersion,
    getTimelineGroups,
  }
})
