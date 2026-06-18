<template>
  <div class="file-tree-wrapper">
    <div class="sidebar-header">
      <div class="sidebar-title">文件结构</div>
      <el-button
        type="primary"
        size="small"
        :icon="FolderOpened"
        @click="showDialog = true"
      >
        选择监控文件夹
      </el-button>
      <div v-if="store.watchedFolder" class="folder-info">
        {{ store.watchedFolder }}
      </div>
    </div>

    <div class="sidebar-content">
      <el-tree
        v-if="store.fileTree.length > 0"
        :data="store.fileTree"
        :props="{ label: 'name', children: 'children' }"
        node-key="path"
        :expand-on-click-node="false"
        :default-expand-all="true"
        :highlight-current="true"
        @node-click="handleNodeClick"
      >
        <template #default="{ node, data }">
          <span class="tree-node" @click.stop="handleNodeClick(data, node)">
            <el-icon
              class="file-icon"
              :style="{ color: getFileIconColor(data) }"
            >
              <component :is="getFileIcon(data)" />
            </el-icon>
            <span class="file-name">{{ data.name }}</span>
          </span>
        </template>
      </el-tree>

      <div v-else class="empty-state">
        <el-icon class="empty-icon"><Folder /></el-icon>
        <span class="empty-text">
          {{ store.watchedFolder ? "加载中..." : "请先选择监控文件夹" }}
        </span>
      </div>
    </div>

    <el-dialog v-model="showDialog" title="选择监控文件夹" width="480px">
      <el-form :model="form" label-width="80px">
        <el-form-item label="文件夹">
          <el-input
            v-model="form.folderPath"
            placeholder="请输入文件夹路径"
            clearable
          >
            <template #append>
              <el-button :icon="Folder" @click="browseFolder" />
            </template>
          </el-input>
        </el-form-item>
      </el-form>
      <template #footer>
        <el-button @click="showDialog = false">取消</el-button>
        <el-button type="primary" :loading="store.isLoading" @click="confirmSelect">
          确定
        </el-button>
      </template>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref, reactive } from "vue"
import {
  ElMessage,
  ElMessageBox,
} from "element-plus"
import {
  Folder,
  FolderOpened,
  Document,
  Notebook,
  Picture,
  EditPen,
  Setting,
  VideoCamera,
  Headset,
  Files,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import type { FileNode } from "@/types"
import { open } from "@tauri-apps/plugin-dialog"

const store = useAppStore()
const showDialog = ref(false)
const form = reactive({
  folderPath: store.watchedFolder || "",
})

const fileIconMap: Record<string, any> = {
  py: EditPen,
  md: Notebook,
  txt: Document,
  js: EditPen,
  ts: EditPen,
  vue: EditPen,
  html: EditPen,
  css: EditPen,
  json: Setting,
  yaml: Setting,
  yml: Setting,
  png: Picture,
  jpg: Picture,
  jpeg: Picture,
  gif: Picture,
  svg: Picture,
  mp4: VideoCamera,
  avi: VideoCamera,
  mov: VideoCamera,
  mp3: Headset,
  wav: Headset,
  flac: Headset,
}

const fileColorMap: Record<string, string> = {
  py: "#3776ab",
  md: "#519aba",
  txt: "#909399",
  js: "#f1e05a",
  ts: "#3178c6",
  vue: "#42b883",
  html: "#e34c26",
  css: "#563d7c",
  json: "#cbcb41",
  yaml: "#cb171e",
  yml: "#cb171e",
  png: "#a074c4",
  jpg: "#a074c4",
  jpeg: "#a074c4",
  gif: "#a074c4",
  svg: "#a074c4",
  mp4: "#e44d26",
  avi: "#e44d26",
  mov: "#e44d26",
  mp3: "#f26d7d",
  wav: "#f26d7d",
  flac: "#f26d7d",
}

function getFileIcon(data: FileNode) {
  if (data.is_dir) return Folder
  const icon = fileIconMap[data.file_type.toLowerCase()]
  return icon || Files
}

function getFileIconColor(data: FileNode) {
  if (data.is_dir) return "#e6a23c"
  return fileColorMap[data.file_type.toLowerCase()] || "#909399"
}

function handleNodeClick(data: FileNode) {
  if (data.is_dir) return
  const fileName = data.name
  store.selectFile(data.path, fileName)
}

async function browseFolder() {
  try {
    const selected = await open({
      directory: true,
      multiple: false,
    })
    if (selected && typeof selected === "string") {
      form.folderPath = selected
    }
  } catch (error) {
    ElMessage.error("选择文件夹失败")
  }
}

async function confirmSelect() {
  if (!form.folderPath.trim()) {
    ElMessage.warning("请输入文件夹路径")
    return
  }
  await store.setWatchedFolder(form.folderPath.trim())
  showDialog.value = false
}
</script>

<style lang="scss" scoped>
.file-tree-wrapper {
  display: flex;
  flex-direction: column;
  height: 100%;
}

:deep(.el-tree) {
  background: transparent;
  --el-tree-node-hover-bg-color: #f5f7fa;
}

:deep(.el-tree-node__content) {
  height: 32px;
  border-radius: 4px;
  margin-bottom: 2px;
}

:deep(.el-tree-node.is-current > .el-tree-node__content) {
  background: rgba(64, 158, 255, 0.1);
  color: #409eff;
}
</style>
