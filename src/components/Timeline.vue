<template>
  <div class="timeline-container">
    <div v-if="!store.selectedFile" class="empty-state" style="padding: 40px 0">
      <el-icon class="empty-icon"><Document /></el-icon>
      <span class="empty-text">请先选择文件以查看版本时间轴</span>
    </div>

    <template v-else>
      <div class="timeline-header">
        <div class="header-info">
          <el-icon style="margin-right: 6px; color: #409eff"><Clock /></el-icon>
          <span style="font-weight: 500">
            {{ store.selectedFileName }}
          </span>
          <el-tag
            size="small"
            type="primary"
            effect="light"
            style="margin-left: 8px"
          >
            共 {{ store.versions.length }} 个版本
          </el-tag>
        </div>
        <div class="header-actions">
          <el-button
            size="small"
            type="primary"
            plain
            :icon="RefreshRight"
            @click="reloadVersions"
          >
            刷新
          </el-button>
        </div>
      </div>

      <div v-if="timelineGroups.length > 0" class="timeline-scroll-area">
        <div
          v-for="(group, gIdx) in timelineGroups"
          :key="group.date"
          class="timeline-group"
        >
          <div class="group-date-header">
            <el-icon style="margin-right: 6px"><Calendar /></el-icon>
            <span class="group-date-text">{{ formatGroupDate(group.date) }}</span>
            <el-tag size="small" effect="plain" style="margin-left: 8px">
              {{ group.items.length }} 次变更
            </el-tag>
          </div>

          <el-timeline>
            <el-timeline-item
              v-for="(item, iIdx) in group.items"
              :key="item.id"
              :timestamp="formatItemTime(item.timestamp)"
              placement="top"
              :color="getTimelineColor(gIdx, iIdx)"
              :hollow="isActiveVersion(item)"
            >
              <div
                class="timeline-card"
                :class="{ active: isActiveVersion(item) }"
                @click="handleClickItem(item)"
              >
                <div class="card-header">
                  <div class="version-label">
                    <el-icon :style="{ color: getVersionIconColor(item) }">
                      <component :is="getVersionIcon(gIdx, iIdx)" />
                    </el-icon>
                    <span class="version-num">版本 {{ item.version_number }}</span>
                    <el-tag
                      v-if="isActiveVersion(item)"
                      size="small"
                      type="success"
                      effect="dark"
                      style="margin-left: 6px"
                    >
                      当前查看
                    </el-tag>
                  </div>
                  <div class="card-actions">
                    <el-button
                      size="small"
                      type="primary"
                      link
                      @click.stop="handleView(item)"
                    >
                      查看内容
                    </el-button>
                    <el-button
                      size="small"
                      type="warning"
                      link
                      @click.stop="handleRestore(item)"
                    >
                      恢复
                    </el-button>
                  </div>
                </div>
                <div v-if="item.message" class="card-message">
                  {{ item.message }}
                </div>
                <div v-else class="card-message card-message-empty">
                  <el-icon style="opacity: 0.5"><ChatDotRound /></el-icon>
                  <span style="opacity: 0.6; font-size: 12px">（无版本备注）</span>
                </div>
                <div class="card-footer">
                  <span class="file-ref">
                    <el-icon><FolderOpened /></el-icon>
                    {{ item.file_name }}
                  </span>
                </div>
              </div>
            </el-timeline-item>
          </el-timeline>
        </div>
      </div>

      <div v-else class="empty-state" style="padding: 40px 0">
        <el-icon class="empty-icon"><DataLine /></el-icon>
        <span class="empty-text">当前文件暂无版本记录</span>
      </div>
    </template>
  </div>
</template>

<script setup lang="ts">
import { computed } from "vue"
import { ElMessageBox } from "element-plus"
import {
  Clock,
  Calendar,
  FolderOpened,
  RefreshRight,
  ChatDotRound,
  Star,
  StarFilled,
  DataLine,
  Document,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import type { TimelineItem } from "@/types"
import moment from "moment"

const store = useAppStore()

const timelineGroups = computed(() => store.getTimelineGroups())

function formatGroupDate(dateStr: string): string {
  const date = moment(dateStr)
  const today = moment().format("YYYY-MM-DD")
  const yesterday = moment().subtract(1, "day").format("YYYY-MM-DD")
  if (dateStr === today) return `今天 (${dateStr})`
  if (dateStr === yesterday) return `昨天 (${dateStr})`
  return `${date.format("dddd")} (${dateStr})`
}

function formatItemTime(ts: string): string {
  return moment(ts).format("HH:mm:ss")
}

function getTimelineColor(gIdx: number, iIdx: number): string {
  if (gIdx === 0 && iIdx === 0) return "#409eff"
  return "#909399"
}

function getVersionIcon(gIdx: number, iIdx: number) {
  if (gIdx === 0 && iIdx === 0) return Star
  if (gIdx === 0 && iIdx === 1) return StarFilled
  return Document
}

function getVersionIconColor(item: TimelineItem): string {
  if (isActiveVersion(item)) return "#67c23a"
  return "#409eff"
}

function isActiveVersion(item: TimelineItem): boolean {
  return store.activeVersion === item.version_number
}

function handleClickItem(item: TimelineItem) {
  handleView(item)
}

async function handleView(item: TimelineItem) {
  const fileName = item.file_name
  if (store.selectedFile !== item.file_path) {
    await store.selectFile(item.file_path, fileName)
  }
  await store.loadFileContent(item.file_path, item.version_number)
}

async function handleRestore(item: TimelineItem) {
  try {
    await ElMessageBox.confirm(
      `确定要将「${item.file_name}」恢复到版本 ${item.version_number} 吗？`,
      "恢复版本确认",
      {
        confirmButtonText: "确定恢复",
        cancelButtonText: "取消",
        type: "warning",
      }
    )
    await store.restoreVersion(item.file_path, item.version_number)
  } catch {
    // 用户取消
  }
}

async function reloadVersions() {
  if (!store.selectedFile) return
  await store.loadFileVersions(store.selectedFile)
}
</script>

<style lang="scss" scoped>
.timeline-container {
  display: flex;
  flex-direction: column;
  height: 100%;
  min-height: 0;
}

.timeline-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: 16px;
  padding: 10px 14px;
  background: #f5f7fa;
  border-radius: 6px;

  .header-info {
    display: flex;
    align-items: center;
    font-size: 14px;
  }
}

.timeline-scroll-area {
  flex: 1;
  overflow: auto;
  padding-right: 8px;
}

.timeline-group {
  margin-bottom: 20px;

  &:last-child {
    margin-bottom: 0;
  }
}

.group-date-header {
  display: flex;
  align-items: center;
  margin-bottom: 12px;
  padding: 6px 10px;
  background: linear-gradient(90deg, #ecf5ff 0%, transparent 100%);
  border-left: 3px solid #409eff;
  border-radius: 0 4px 4px 0;
  font-size: 13px;
  color: #303133;

  .group-date-text {
    font-weight: 500;
  }
}

.timeline-card {
  padding: 12px 14px;
  background: #fff;
  border: 1px solid #ebeef5;
  border-radius: 8px;
  transition: all 0.2s;
  cursor: pointer;

  &:hover {
    border-color: #409eff;
    box-shadow: 0 2px 12px rgba(64, 158, 255, 0.12);
    transform: translateY(-1px);
  }

  &.active {
    border-color: #67c23a;
    background: #f0f9eb;
  }

  .card-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 8px;

    .version-label {
      display: flex;
      align-items: center;
      gap: 6px;

      .version-num {
        font-weight: 600;
        color: #303133;
        font-size: 14px;
      }
    }

    .card-actions {
      display: flex;
      gap: 4px;
    }
  }

  .card-message {
    font-size: 13px;
    color: #606266;
    line-height: 1.5;
    padding: 6px 10px;
    background: #fafafa;
    border-radius: 4px;
    margin-bottom: 8px;

    &.card-message-empty {
      color: #c0c4cc;
      display: flex;
      align-items: center;
      gap: 6px;
      background: transparent;
      padding: 2px 0;
    }
  }

  .card-footer {
    font-size: 12px;
    color: #909399;

    .file-ref {
      display: inline-flex;
      align-items: center;
      gap: 4px;
    }
  }
}
</style>
