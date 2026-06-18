<template>
  <div class="sync-panel">
    <div class="sync-mode-section">
      <el-tabs v-model="activeMode" class="mode-tabs">
        <el-tab-pane name="server" disabled>
          <template #label>
            <el-icon style="margin-right: 4px"><Monitor /></el-icon>
            启动服务器
          </template>
        </el-tab-pane>
        <el-tab-pane name="client" disabled>
          <template #label>
            <el-icon style="margin-right: 4px"><Link /></el-icon>
            连接服务器
          </template>
        </el-tab-pane>
      </el-tabs>

      <div v-if="store.syncMode === 'none'" class="mode-config">
        <div v-if="activeMode === 'server'" class="server-config">
          <el-input-number
            v-model="serverPort"
            :min="1024"
            :max="65535"
            size="default"
            style="width: 180px"
          />
          <el-button
            type="primary"
            :loading="store.isLoading"
            @click="handleStartServer"
          >
            <el-icon style="margin-right: 4px"><VideoPlay /></el-icon>
            启动
          </el-button>
        </div>
        <div v-else class="client-config">
          <el-input
            v-model="serverAddress"
            placeholder="例如: 192.168.1.100:9527"
            style="width: 280px"
            clearable
          />
          <el-button
            type="primary"
            :loading="store.isLoading"
            @click="handleConnect"
          >
            <el-icon style="margin-right: 4px"><Connection /></el-icon>
            连接
          </el-button>
        </div>
      </div>

      <div v-else class="connected-info">
        <el-tag :type="store.syncMode === 'server' ? 'success' : 'primary'" effect="dark">
          {{ store.syncMode === "server" ? "服务器模式" : "客户端模式" }}
        </el-tag>
        <span class="addr-text">{{ store.syncServerAddr }}</span>
        <el-button type="danger" size="small" plain @click="handleDisconnect">
          <el-icon style="margin-right: 4px"><SwitchButton /></el-icon>
          断开
        </el-button>
      </div>
    </div>

    <div class="sync-status-section">
      <div class="status-row">
        <div class="status-item">
          <span class="status-dot" :class="connectionClass"></span>
          <span class="status-label">{{ connectionLabel }}</span>
        </div>
        <div class="status-item">
          <el-icon style="margin-right: 4px; color: #409eff"><User /></el-icon>
          <span>对等节点: <b>{{ store.syncStats.connected_clients }}</b></span>
        </div>
        <div class="status-item">
          <el-icon style="margin-right: 4px; color: #67c23a"><Document /></el-icon>
          <span>已同步: <b>{{ store.syncStats.files_synced }}</b> 文件</span>
        </div>
        <div class="status-item">
          <el-icon style="margin-right: 4px; color: #e6a23c"><Grid /></el-icon>
          <span>已传输: <b>{{ store.syncStats.blocks_transferred }}</b> 块 / {{ formatBytes(store.syncStats.bytes_transferred) }}</span>
        </div>
        <div class="status-item" v-if="store.syncStats.last_sync_time">
          <el-icon style="margin-right: 4px; color: #909399"><Clock /></el-icon>
          <span style="color: #909399; font-size: 12px">最后同步: {{ formatTime(store.syncStats.last_sync_time!) }}</span>
        </div>
        <el-button
          type="primary"
          size="small"
          :loading="store.isSyncing"
          :disabled="store.syncMode === 'none'"
          @click="store.syncNow()"
        >
          <el-icon style="margin-right: 4px"><Refresh /></el-icon>
          立即同步
        </el-button>
      </div>
    </div>

    <div class="sync-tables-section">
      <el-tabs v-model="activeTable" class="table-tabs">
        <el-tab-pane name="conflicts">
          <template #label>
            <el-icon style="margin-right: 4px"><Warning /></el-icon>
            冲突列表
            <el-badge
              v-if="store.conflicts.length > 0"
              :value="store.conflicts.length"
              class="tab-badge"
            />
          </template>
        </el-tab-pane>
        <el-tab-pane name="peers">
          <template #label>
            <el-icon style="margin-right: 4px"><User /></el-icon>
            对等节点
          </template>
        </el-tab-pane>
      </el-tabs>

      <el-table
        v-if="activeTable === 'conflicts'"
        :data="store.conflicts"
        stripe
        size="small"
        style="width: 100%"
        max-height="140"
      >
        <el-table-column prop="file_path" label="文件路径" min-width="180">
          <template #default="{ row }">
            <span style="font-size: 12px; color: #303133" :title="row.file_path">
              {{ row.file_path }}
            </span>
          </template>
        </el-table-column>
        <el-table-column label="本地版本" min-width="160">
          <template #default="{ row }">
            <div style="font-size: 12px; line-height: 1.4">
              <div style="color: #606266">{{ formatTime(row.local_node.timestamp) }}</div>
              <div style="color: #909399; font-family: monospace; font-size: 11px">
                {{ row.local_node.content_hash.slice(0, 12) }}
              </div>
            </div>
          </template>
        </el-table-column>
        <el-table-column label="远程版本" min-width="180">
          <template #default="{ row }">
            <div style="font-size: 12px; line-height: 1.4">
              <div style="color: #606266">{{ formatTime(row.remote_node.timestamp) }}</div>
              <div style="color: #909399; font-family: monospace; font-size: 11px">
                {{ row.remote_node.content_hash.slice(0, 12) }} · {{ row.remote_node.client_id.slice(0, 8) }}
              </div>
            </div>
          </template>
        </el-table-column>
        <el-table-column prop="resolution_strategy" label="解决策略" width="120" align="center">
          <template #default="{ row }">
            <el-tag size="small" :type="row.needs_manual_merge ? 'danger' : 'info'">
              {{ row.needs_manual_merge ? "需手动合并" : row.resolution_strategy }}
            </el-tag>
          </template>
        </el-table-column>
        <el-table-column label="操作" width="200" align="center">
          <template #default="{ row }">
            <el-button
              size="small"
              type="primary"
              plain
              @click="store.resolveConflict(row.file_path, true)"
            >
              使用本地
            </el-button>
            <el-button
              size="small"
              type="warning"
              plain
              @click="store.resolveConflict(row.file_path, false)"
            >
              使用远程
            </el-button>
          </template>
        </el-table-column>
      </el-table>

      <el-table
        v-else
        :data="store.connectedPeers"
        stripe
        size="small"
        style="width: 100%"
        max-height="140"
      >
        <el-table-column prop="client_id" label="客户端ID" min-width="140">
          <template #default="{ row }">
            <span style="font-size: 12px; font-family: monospace">{{ row.client_id }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="hostname" label="主机名" min-width="120">
          <template #default="{ row }">
            <span style="font-size: 12px; color: #303133">{{ row.hostname }}</span>
          </template>
        </el-table-column>
        <el-table-column label="地址" min-width="160">
          <template #default="{ row }">
            <span style="font-size: 12px; color: #606266">{{ row.address }}:{{ row.listen_port }}</span>
          </template>
        </el-table-column>
        <el-table-column prop="version_count" label="版本数" width="100" align="center">
          <template #default="{ row }">
            <el-tag size="small" type="info">{{ row.version_count }}</el-tag>
          </template>
        </el-table-column>
      </el-table>

      <div
        v-if="activeTable === 'conflicts' && store.conflicts.length === 0"
        class="empty-hint"
      >
        <el-icon style="font-size: 20px; color: #c0c4cc"><CircleCheck /></el-icon>
        <span>暂无冲突</span>
      </div>
      <div
        v-if="activeTable === 'peers' && store.connectedPeers.length === 0"
        class="empty-hint"
      >
        <el-icon style="font-size: 20px; color: #c0c4cc"><User /></el-icon>
        <span>暂无对等节点</span>
      </div>
    </div>
  </div>
</template>

<script setup lang="ts">
import { ref, computed } from "vue"
import {
  Monitor,
  Link,
  VideoPlay,
  Connection,
  SwitchButton,
  User,
  Document,
  Grid,
  Clock,
  Refresh,
  Warning,
  CircleCheck,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import moment from "moment"

const store = useAppStore()

const activeMode = ref<"server" | "client">("server")
const serverPort = ref(9527)
const serverAddress = ref("")
const activeTable = ref<"conflicts" | "peers">("conflicts")

const connectionClass = computed(() => {
  if (store.syncMode === "none") return "disconnected"
  return "connected"
})

const connectionLabel = computed(() => {
  if (store.syncMode === "none") return "未连接"
  if (store.syncMode === "server") return "服务器运行中"
  return "已连接"
})

function formatTime(ts: string): string {
  return moment(ts).format("MM-DD HH:mm:ss")
}

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B"
  const units = ["B", "KB", "MB", "GB"]
  const i = Math.floor(Math.log(bytes) / Math.log(1024))
  return (bytes / Math.pow(1024, i)).toFixed(1) + " " + units[i]
}

async function handleStartServer() {
  await store.startServer(serverPort.value)
}

async function handleConnect() {
  if (!serverAddress.value.trim()) return
  await store.connectToServer(serverAddress.value.trim())
}

async function handleDisconnect() {
  await store.disconnectSync()
}
</script>

<style lang="scss" scoped>
.sync-panel {
  display: flex;
  flex-direction: column;
  gap: 12px;
  height: 100%;
}

.sync-mode-section {
  .mode-tabs {
    :deep(.el-tabs__header) {
      margin: 0;
    }

    :deep(.el-tabs__nav-wrap::after) {
      height: 1px;
    }

    :deep(.el-tabs__item) {
      height: 36px;
      line-height: 36px;
      font-size: 13px;
    }
  }

  .mode-config {
    display: flex;
    align-items: center;
    padding: 8px 0;

    .server-config,
    .client-config {
      display: flex;
      align-items: center;
      gap: 12px;
    }
  }

  .connected-info {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 8px 0;

    .addr-text {
      font-size: 13px;
      color: #606266;
      font-family: monospace;
    }
  }
}

.sync-status-section {
  .status-row {
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 8px 12px;
    background: #f5f7fa;
    border-radius: 6px;
    flex-wrap: wrap;

    .status-item {
      display: flex;
      align-items: center;
      font-size: 13px;
      color: #303133;
      white-space: nowrap;
    }

    .status-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      margin-right: 6px;

      &.connected {
        background: #67c23a;
        box-shadow: 0 0 6px #67c23a;
      }

      &.disconnected {
        background: #c0c4cc;
      }
    }
  }
}

.sync-tables-section {
  flex: 1;
  min-height: 0;
  display: flex;
  flex-direction: column;

  .table-tabs {
    :deep(.el-tabs__header) {
      margin: 0;
    }

    :deep(.el-tabs__nav-wrap::after) {
      height: 1px;
    }

    :deep(.el-tabs__item) {
      height: 32px;
      line-height: 32px;
      font-size: 13px;
    }
  }

  .tab-badge {
    margin-left: 6px;

    :deep(.el-badge__content) {
      font-size: 10px;
      height: 16px;
      line-height: 16px;
      padding: 0 5px;
    }
  }

  .empty-hint {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    padding: 20px 0;
    color: #c0c4cc;
    font-size: 13px;
  }
}
</style>
