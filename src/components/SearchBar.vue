<template>
  <div class="search-bar-container">
    <div class="search-input-wrapper">
      <el-input
        v-model="query"
        size="large"
        placeholder="输入自然语言搜索内容，例如：查找包含用户登录逻辑的代码"
        clearable
        @keyup.enter="handleSearch"
        @clear="clearSearch"
      >
        <template #prefix>
          <el-icon><Search /></el-icon>
        </template>
        <template #append>
          <el-button
            type="primary"
            size="large"
            :loading="store.isLoading"
            @click="handleSearch"
          >
            <el-icon style="margin-right: 4px"><Search /></el-icon>
            语义搜索
          </el-button>
        </template>
      </el-input>
    </div>

    <div class="filters-toggle">
      <el-checkbox v-model="showFilters">
        <el-icon style="margin-right: 4px"><Filter /></el-icon>
        高级过滤选项
      </el-checkbox>
    </div>

    <div v-show="showFilters" class="filters-section">
      <div class="filter-row">
        <span class="filter-label">日期范围:</span>
        <el-date-picker
          v-model="dateRange"
          type="daterange"
          range-separator="至"
          start-placeholder="开始日期"
          end-placeholder="结束日期"
          value-format="YYYY-MM-DD"
          size="default"
          style="width: 360px"
        />
      </div>
      <div class="filter-row">
        <span class="filter-label">文件类型:</span>
        <el-select
          v-model="selectedFileTypes"
          multiple
          collapse-tags
          collapse-tags-tooltip
          placeholder="选择文件类型（留空表示全部）"
          size="default"
          style="width: 360px"
        >
          <el-option
            v-for="ft in fileTypeOptions"
            :key="ft.value"
            :label="ft.label"
            :value="ft.value"
          />
        </el-select>
      </div>
      <div class="filter-row">
        <span class="filter-label">关键字:</span>
        <el-select
          v-model="keywords"
          multiple
          filterable
          allow-create
          default-first-option
          placeholder="输入关键字后按回车添加（可选）"
          size="default"
          style="width: 360px"
        />
      </div>
    </div>

    <div class="results-wrapper">
      <div v-if="store.searchResults.length > 0" class="results-header">
        <el-icon style="margin-right: 6px; color: #409eff"><Search /></el-icon>
        <span>找到 <b style="color: #409eff">{{ store.searchResults.length }}</b> 条匹配结果</span>
        <el-divider direction="vertical" />
        <span style="color: #909399; font-size: 12px">
          按相关度排序，点击结果可打开文件
        </span>
      </div>

      <el-table
        v-if="store.searchResults.length > 0"
        :data="store.searchResults"
        stripe
        style="width: 100%"
        height="260"
        @row-click="handleRowClick"
      >
        <el-table-column prop="file_path" label="文件路径" min-width="260">
          <template #default="{ row }">
            <div class="table-path">
              <el-icon style="color: #409eff; margin-right: 6px; flex-shrink: 0">
                <Document />
              </el-icon>
              <span class="path-text" :title="row.file_path">
                {{ row.file_path }}
              </span>
              <el-tag
                v-if="row.version_number"
                size="small"
                type="info"
                effect="plain"
                style="margin-left: 8px; flex-shrink: 0"
              >
                v{{ row.version_number }}
              </el-tag>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="score" label="相关度" width="100" align="center">
          <template #default="{ row }">
            <el-rate
              :model-value="Math.round(row.score * 5)"
              disabled
              size="small"
              show-score
              text-color="#ff9900"
              score-template="{value}"
            />
          </template>
        </el-table-column>
        <el-table-column prop="timestamp" label="修改时间" width="160" align="center">
          <template #default="{ row }">
            <span style="font-size: 12px; color: #606266">
              {{ formatTime(row.timestamp) }}
            </span>
          </template>
        </el-table-column>
        <el-table-column label="文件类型" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="getFileTypeTag(row.file_type)">
              {{ row.file_type.toUpperCase() }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column prop="snippet" label="内容片段" min-width="200">
          <template #default="{ row }">
            <span
              style="font-size: 12px; color: #606266; line-height: 1.5"
              class="snippet-text"
            >
              {{ row.snippet }}
            </span>
          </template>
        </el-table-column>
      </el-table>

      <div v-else-if="!query.trim()" class="empty-state" style="padding: 40px 0">
        <el-icon class="empty-icon"><Search /></el-icon>
        <span class="empty-text">输入关键词开始搜索</span>
      </div>
      <div v-else-if="store.isLoading" class="empty-state" style="padding: 40px 0">
        <el-icon class="empty-icon" style="color: #409eff"><Loading /></el-icon>
        <span class="empty-text">正在搜索中...</span>
      </div>
      <div v-else class="empty-state" style="padding: 40px 0">
        <el-icon class="empty-icon"><CircleClose /></el-icon>
        <span class="empty-text">未找到匹配的内容，请尝试其他关键词</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue"
import {
  Search,
  Filter,
  Document,
  CircleClose,
  Loading,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import type { SearchFilters } from "@/types"
import moment from "moment"

const store = useAppStore()

const query = ref("")
const showFilters = ref(false)
const dateRange = ref<string[]>([])
const selectedFileTypes = ref<string[]>([])
const keywords = ref<string[]>([])

const fileTypeOptions = [
  { label: "Python (.py)", value: "py" },
  { label: "Markdown (.md)", value: "md" },
  { label: "文本 (.txt)", value: "txt" },
  { label: "JavaScript (.js)", value: "js" },
  { label: "TypeScript (.ts)", value: "ts" },
  { label: "Vue (.vue)", value: "vue" },
  { label: "HTML (.html)", value: "html" },
  { label: "CSS (.css)", value: "css" },
  { label: "JSON (.json)", value: "json" },
  { label: "YAML (.yaml/.yml)", value: "yaml" },
  { label: "图片 (.png/.jpg)", value: "image" },
  { label: "其他", value: "other" },
]

function formatTime(ts: string): string {
  return moment(ts).format("YYYY-MM-DD HH:mm:ss")
}

function getFileTypeTag(ft: string): string {
  const map: Record<string, string> = {
    py: "success",
    md: "warning",
    txt: "info",
    js: "warning",
    ts: "primary",
    vue: "success",
    html: "danger",
    css: "",
    json: "warning",
    yaml: "info",
  }
  return map[ft.toLowerCase()] || "info"
}

const filters = computed<SearchFilters>(() => {
  const f: SearchFilters = {}
  if (dateRange.value.length === 2) {
    f.date_from = dateRange.value[0]
    f.date_to = dateRange.value[1]
  }
  if (selectedFileTypes.value.length > 0) {
    f.file_types = selectedFileTypes.value
  }
  if (keywords.value.length > 0) {
    f.keywords = keywords.value
  }
  return f
})

async function handleSearch() {
  await store.performSearch(query.value, filters.value)
}

function clearSearch() {
  store.searchResults = []
}

function handleRowClick(row: any) {
  const fileName = row.file_path.split(/[/\\]/).pop() || ""
  store.selectFile(row.file_path, fileName)
  if (row.version_number !== undefined && row.version_number !== null) {
    store.loadFileContent(row.file_path, row.version_number)
  }
}
</script>

<style lang="scss" scoped>
.filters-toggle {
  margin-bottom: 12px;

  :deep(.el-checkbox__label) {
    font-size: 13px;
    color: #606266;
  }
}

.results-wrapper {
  .results-header {
    display: flex;
    align-items: center;
    margin-bottom: 12px;
    padding: 8px 12px;
    background: #ecf5ff;
    border-radius: 4px;
    font-size: 13px;
    color: #606266;
  }
}

.table-path {
  display: flex;
  align-items: center;
  width: 100%;
  overflow: hidden;

  .path-text {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-size: 13px;
    color: #303133;
  }
}

:deep(.el-table__row) {
  cursor: pointer;
}

.snippet-text {
  display: -webkit-box;
  -webkit-line-clamp: 2;
  -webkit-box-orient: vertical;
  overflow: hidden;
}
</style>
