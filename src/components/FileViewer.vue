<template>
  <div class="file-viewer">
    <div v-if="!store.selectedFile" class="empty-state" style="height: 100%">
      <el-icon class="empty-icon"><Document /></el-icon>
      <span class="empty-text">请从左侧选择文件查看</span>
    </div>

    <template v-else>
      <div class="viewer-header">
        <div class="file-title">
          <el-icon><Document /></el-icon>
          {{ store.selectedFileName }}
          <el-tag
            v-if="store.activeVersion"
            size="small"
            type="primary"
            effect="dark"
          >
            版本 {{ store.activeVersion }}
          </el-tag>
          <el-tag v-else size="small" type="success" effect="light">
            当前版本
          </el-tag>
        </div>
        <div class="file-meta">
          <span>{{ store.selectedFile }}</span>
          <el-divider direction="vertical" />
          <span>共 {{ store.versions.length }} 个历史版本</span>
        </div>
      </div>

      <div class="viewer-tabs">
        <el-tabs v-model="activeTab" @tab-change="handleTabChange">
          <el-tab-pane label="当前内容" name="content" />
          <el-tab-pane label="版本历史" name="history" />
          <el-tab-pane label="Diff 对比" name="diff" />
        </el-tabs>
      </div>

      <div class="viewer-content">
        <div v-if="activeTab === 'content'">
          <div v-if="store.fileContent" class="code-block">
            <pre><code>{{ store.fileContent }}</code></pre>
          </div>
          <div v-else class="empty-state" style="padding: 40px 0">
            <el-icon class="empty-icon"><Reading /></el-icon>
            <span class="empty-text">文件内容为空或读取失败</span>
          </div>
        </div>

        <div v-else-if="activeTab === 'history'">
          <div v-if="store.versions.length > 0">
            <div
              v-for="v in sortedVersions"
              :key="v.id"
              class="version-item"
              :class="{ active: store.activeVersion === v.version_number }"
              @click="viewVersion(v)"
            >
              <div class="version-header">
                <span class="version-number">
                  <el-icon><Clock /></el-icon>
                  版本 {{ v.version_number }}
                </span>
                <span class="version-time">{{ formatTime(v.timestamp) }}</span>
              </div>
              <div v-if="v.message" class="version-message">
                {{ v.message }}
              </div>
              <div class="version-meta">
                <span>
                  <el-icon><Coin /></el-icon>
                  {{ formatSize(v.size) }}
                </span>
                <span>
                  <el-icon><Connection /></el-icon>
                  {{ v.diff_patch ? "有差异" : "无变更" }}
                </span>
                <div style="flex: 1"></div>
                <el-button
                  v-if="store.activeVersion !== v.version_number"
                  size="small"
                  type="primary"
                  link
                  @click.stop="viewVersion(v)"
                >
                  查看
                </el-button>
                <el-button
                  size="small"
                  type="warning"
                  link
                  @click.stop="handleRestore(v)"
                >
                  恢复到此版本
                </el-button>
              </div>
            </div>
          </div>
          <div v-else class="empty-state" style="padding: 40px 0">
            <el-icon class="empty-icon"><DataLine /></el-icon>
            <span class="empty-text">暂无历史版本记录</span>
          </div>
        </div>

        <div v-else-if="activeTab === 'diff'">
          <div v-if="store.versions.length >= 1" class="diff-container">
            <div class="diff-selector">
              <div class="selector-item">
                <span class="selector-label">基础版本:</span>
                <el-select
                  v-model="baseVersion"
                  placeholder="选择版本"
                  style="width: 180px"
                  size="small"
                >
                  <el-option
                    label="当前版本"
                    :value="null"
                  />
                  <el-option
                    v-for="v in sortedVersions"
                    :key="v.id"
                    :label="`版本 ${v.version_number}`"
                    :value="v.version_number"
                  />
                </el-select>
              </div>
              <el-icon style="color: #909399; margin: 0 12px"><Right /></el-icon>
              <div class="selector-item">
                <span class="selector-label">对比版本:</span>
                <el-select
                  v-model="compareVersion"
                  placeholder="选择版本"
                  style="width: 180px"
                  size="small"
                >
                  <el-option
                    v-for="v in sortedVersions"
                    :key="v.id"
                    :label="`版本 ${v.version_number}`"
                    :value="v.version_number"
                  />
                </el-select>
              </div>
              <el-button
                type="primary"
                size="small"
                :disabled="compareVersion === null"
                @click="computeDiff"
              >
                计算差异
              </el-button>
            </div>

            <div v-if="diffLines.length > 0" class="diff-block">
              <div
                v-for="(line, idx) in diffLines"
                :key="idx"
                class="diff-line"
                :class="line.type"
              >
                <span class="line-prefix">{{ line.prefix }}</span>
                {{ line.text }}
              </div>
            </div>
            <div v-else-if="diffComputed" class="empty-state" style="padding: 40px 0">
              <el-icon class="empty-icon"><CircleCheck /></el-icon>
              <span class="empty-text">两个版本内容完全相同</span>
            </div>
            <div v-else class="empty-state" style="padding: 40px 0">
              <el-icon class="empty-icon"><Sort /></el-icon>
              <span class="empty-text">请选择版本并点击「计算差异」</span>
            </div>
          </div>
          <div v-else class="empty-state" style="padding: 40px 0">
            <el-icon class="empty-icon"><DataLine /></el-icon>
            <span class="empty-text">版本不足，无法进行对比</span>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { ref, computed, watch } from "vue"
import { ElMessageBox } from "element-plus"
import {
  Document,
  Clock,
  Coin,
  Connection,
  Reading,
  DataLine,
  Right,
  CircleCheck,
  Sort,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import * as Diff from "diff"
import type { VersionInfo } from "@/types"
import moment from "moment"

const store = useAppStore()
const activeTab = ref("content")
const baseVersion = ref<number | null>(null)
const compareVersion = ref<number | null>(null)
const diffLines = ref<
  Array<{ type: string; prefix: string; text: string }>
>([])
const diffComputed = ref(false)

const sortedVersions = computed(() => {
  return [...store.versions].sort(
    (a, b) => b.version_number - a.version_number
  )
})

watch(
  () => store.selectedFile,
  () => {
    activeTab.value = "content"
    diffLines.value = []
    diffComputed.value = false
    baseVersion.value = null
    if (sortedVersions.value.length > 0) {
      compareVersion.value = sortedVersions.value[0].version_number
    } else {
      compareVersion.value = null
    }
  }
)

watch(
  () => sortedVersions.value,
  (versions) => {
    if (versions.length > 0 && compareVersion.value === null) {
      compareVersion.value = versions[0].version_number
    }
  }
)

function handleTabChange(tab: string) {
  activeTab.value = tab
  if (tab === "diff") {
    diffLines.value = []
    diffComputed.value = false
  }
}

function formatTime(ts: string): string {
  return moment(ts).format("YYYY-MM-DD HH:mm:ss")
}

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(2)} KB`
  return `${(bytes / (1024 * 1024)).toFixed(2)} MB`
}

async function viewVersion(v: VersionInfo) {
  await store.loadFileContent(v.file_path, v.version_number)
}

async function handleRestore(v: VersionInfo) {
  try {
    await ElMessageBox.confirm(
      `确定要将文件恢复到版本 ${v.version_number} 吗？当前内容将被覆盖。`,
      "恢复版本确认",
      {
        confirmButtonText: "确定恢复",
        cancelButtonText: "取消",
        type: "warning",
      }
    )
    await store.restoreVersion(v.file_path, v.version_number)
    activeTab.value = "content"
  } catch {
    // 用户取消
  }
}

async function computeDiff() {
  if (compareVersion.value === null) return
  try {
    diffComputed.value = false
    diffLines.value = []

    let baseText: string
    if (baseVersion.value === null) {
      baseText = await getCurrentContent()
    } else {
      baseText = await store.loadFileContent(
        store.selectedFile,
        baseVersion.value
      )
      baseText = store.fileContent
    }

    const compareText =
      baseVersion.value === compareVersion.value
        ? baseText
        : await (async () => {
            await store.loadFileContent(store.selectedFile, compareVersion.value!)
            return store.fileContent
          })()

    const changes = Diff.diffLines(baseText, compareText)
    const lines: Array<{ type: string; prefix: string; text: string }> = []

    for (const part of changes) {
      const textLines = part.value.split("\n")
      if (textLines.length > 0 && textLines[textLines.length - 1] === "") {
        textLines.pop()
      }
      for (const line of textLines) {
        if (part.added) {
          lines.push({ type: "diff-add", prefix: "+", text: line })
        } else if (part.removed) {
          lines.push({ type: "diff-remove", prefix: "-", text: line })
        } else {
          lines.push({ type: "diff-context", prefix: " ", text: line })
        }
      }
    }

    diffLines.value = lines
    diffComputed.value = true
  } catch (error) {
    console.error("Diff 计算失败:", error)
  }
}

async function getCurrentContent(): Promise<string> {
  const prevVersion = store.activeVersion
  await store.loadFileContent(store.selectedFile)
  const content = store.fileContent
  if (prevVersion !== null) {
    await store.loadFileContent(store.selectedFile, prevVersion)
  }
  return content
}
</script>

<style lang="scss" scoped>
.viewer-tabs {
  border-bottom: 1px solid #ebeef5;
  padding: 0 20px;

  :deep(.el-tabs__header) {
    margin-bottom: 0;
  }

  :deep(.el-tabs__item) {
    height: 44px;
    line-height: 44px;
  }
}

.diff-container {
  .diff-selector {
    display: flex;
    align-items: center;
    margin-bottom: 16px;
    padding: 12px 16px;
    background: #f5f7fa;
    border-radius: 6px;

    .selector-item {
      display: flex;
      align-items: center;
      gap: 8px;

      .selector-label {
        font-size: 13px;
        color: #606266;
      }
    }
  }
}

.line-prefix {
  display: inline-block;
  width: 20px;
  text-align: center;
  font-weight: bold;
}
</style>
