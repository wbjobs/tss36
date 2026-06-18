<template>
  <div class="app-container">
    <header class="app-header">
      <div class="header-title">
        <el-icon class="logo-icon"><Files /></el-icon>
        <span>DocVersion - 文档版本控制与语义搜索</span>
      </div>
      <div class="header-actions">
        <div class="status-indicator">
          <span class="status-dot" :class="{ active: store.isWatching }"></span>
          <span>{{ store.isWatching ? "监控中" : "未监控" }}</span>
        </div>
        <el-tooltip content="刷新文件树" placement="bottom">
          <el-button
            :icon="Refresh"
            circle
            size="small"
            :loading="store.isLoading"
            @click="store.refreshTree"
          />
        </el-tooltip>
        <el-dropdown @command="handleCommand">
          <el-button :icon="MoreFilled" circle size="small" />
          <template #dropdown>
            <el-dropdown-menu>
              <el-dropdown-item command="help">使用说明</el-dropdown-item>
              <el-dropdown-item command="about">关于</el-dropdown-item>
            </el-dropdown-menu>
          </template>
        </el-dropdown>
      </div>
    </header>

    <main class="app-main">
      <aside class="sidebar">
        <FileTree />
      </aside>

      <section class="content-area">
        <div style="flex: 1; display: flex; flex-direction: column; min-height: 0">
          <FileViewer />
        </div>

        <div class="bottom-panel">
          <div class="panel-tabs">
            <el-tabs
              v-model="bottomPanelActive"
              class="bottom-tabs"
              @tab-change="handleBottomTabChange"
            >
              <el-tab-pane name="search">
                <template #label>
                  <el-icon style="margin-right: 4px"><Search /></el-icon>
                  语义搜索
                </template>
              </el-tab-pane>
              <el-tab-pane name="timeline">
                <template #label>
                  <el-icon style="margin-right: 4px"><Clock /></el-icon>
                  历史时间轴
                </template>
              </el-tab-pane>
            </el-tabs>
          </div>
          <div class="panel-content">
            <SearchBar v-if="bottomPanelActive === 'search'" />
            <Timeline v-else />
          </div>
        </div>
      </section>
    </main>

    <el-dialog v-model="showHelpDialog" title="使用说明" width="560px">
      <div style="line-height: 1.8; color: #606266">
        <h4 style="margin: 0 0 8px; color: #303133">📁 文件监控</h4>
        <p style="margin: 0 0 12px; padding-left: 8px">
          点击左侧「选择监控文件夹」按钮，选择需要进行版本控制的文件夹，系统将自动监控文件变更。
        </p>

        <h4 style="margin: 0 0 8px; color: #303133">📄 查看文件内容</h4>
        <p style="margin: 0 0 12px; padding-left: 8px">
          在左侧文件树中点击任意文件，右侧将显示文件内容、版本历史和 Diff 对比三个标签页。
        </p>

        <h4 style="margin: 0 0 8px; color: #303133">🕐 版本恢复</h4>
        <p style="margin: 0 0 12px; padding-left: 8px">
          在「版本历史」或底部「历史时间轴」中，可查看所有历史版本并恢复到任意版本。
        </p>

        <h4 style="margin: 0 0 8px; color: #303133">🔍 语义搜索</h4>
        <p style="margin: 0 0 12px; padding-left: 8px">
          使用底部「语义搜索」，输入自然语言描述（如：查找用户登录逻辑），可按相关度检索文件和版本。
        </p>

        <h4 style="margin: 0 0 8px; color: #303133">📊 Diff 对比</h4>
        <p style="margin: 0 0 12px; padding-left: 8px">
          在文件查看器的「Diff 对比」标签页中，选择两个版本即可查看行级差异（绿色新增、红色删除）。
        </p>
      </div>
      <template #footer>
        <el-button type="primary" @click="showHelpDialog = false">我知道了</el-button>
      </template>
    </el-dialog>

    <el-dialog v-model="showAboutDialog" title="关于 DocVersion" width="420px">
      <div style="text-align: center; padding: 20px 0">
        <el-icon style="font-size: 48px; color: #409eff"><Files /></el-icon>
        <h2 style="margin: 12px 0 6px; color: #303133">DocVersion</h2>
        <p style="color: #909399; margin-bottom: 16px">文档版本控制与语义搜索系统</p>
        <el-divider />
        <div style="text-align: left; line-height: 2; color: #606266; font-size: 13px">
          <p>版本: <b>0.1.0</b></p>
          <p>技术栈: Vue 3 + TypeScript + Element Plus + Pinia</p>
          <p>桌面框架: Tauri 2.0</p>
          <p>License: MIT</p>
        </div>
      </div>
    </el-dialog>
  </div>
</template>

<script setup lang="ts">
import { ref } from "vue"
import {
  Files,
  Refresh,
  MoreFilled,
  Search,
  Clock,
} from "@element-plus/icons-vue"
import { useAppStore } from "@/stores/app"
import FileTree from "@/components/FileTree.vue"
import FileViewer from "@/components/FileViewer.vue"
import SearchBar from "@/components/SearchBar.vue"
import Timeline from "@/components/Timeline.vue"

const store = useAppStore()
const bottomPanelActive = ref("search")
const showHelpDialog = ref(false)
const showAboutDialog = ref(false)

function handleBottomTabChange(tab: string) {
  bottomPanelActive.value = tab
}

function handleCommand(cmd: string) {
  if (cmd === "help") {
    showHelpDialog.value = true
  } else if (cmd === "about") {
    showAboutDialog.value = true
  }
}
</script>

<style lang="scss" scoped>
.bottom-tabs {
  :deep(.el-tabs__header) {
    margin: 0;
    padding: 0 16px;
  }

  :deep(.el-tabs__nav-wrap::after) {
    height: 1px;
  }

  :deep(.el-tabs__item) {
    height: 42px;
    line-height: 42px;
  }
}
</style>
