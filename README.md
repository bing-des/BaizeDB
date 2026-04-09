# BaizeDB

> 基于 Rust + Tauri v2 + React 的现代化 PC 端数据库管理工具

## 功能特性

- ✅ **MySQL / PostgreSQL** 连接管理（测试连接、保存、断开）
- ✅ **数据库浏览器** — 库 / 表 / 列树形视图，懒加载
- ✅ **SQL 编辑器** — CodeMirror 语法高亮（MySQL/PG 方言）、Ctrl+Enter 执行
- ✅ **结果表格** — NULL/数字/布尔高亮、行号、CSV 导出
- ✅ **表数据查看** — 分页浏览（200行/页）、列结构面板
- ✅ **多标签页** — 查询标签 + 表标签同时开启
- ✅ **亮色 / 暗色 / 系统主题** 一键切换

## 技术栈

| 层     | 技术                                          |
|--------|-----------------------------------------------|
| 后端   | Rust + Tauri v2 + sqlx 0.8 (MySQL + PostgreSQL)|
| 前端   | React 18 + TypeScript + Tailwind CSS v3       |
| 编辑器 | CodeMirror 6 via @uiw/react-codemirror        |
| 状态   | Zustand 5                                     |
| 布局   | react-resizable-panels                        |
| 图标   | lucide-react                                  |

## 快速开始

### 前置依赖

- [Rust](https://rustup.rs/) 1.70+
- [Node.js](https://nodejs.org/) 18+
- Windows: Visual Studio C++ Build Tools

### 开发模式

```bash
npm install
npm run tauri dev
```

### 构建发布包

```bash
npm run tauri build
```

输出在 `src-tauri/target/release/bundle/`

## 项目结构

```
BaizeDB/
├── src/                      # React 前端
│   ├── components/
│   │   ├── layout/           # MainLayout、TitleBar
│   │   ├── sidebar/          # 连接树（Sidebar、ConnectionTree）
│   │   ├── connection/       # 新建连接对话框
│   │   ├── editor/           # SQL编辑器、标签页、结果表格
│   │   └── table/            # 表格查看器
│   ├── store/                # Zustand stores
│   ├── types/                # TypeScript 类型定义
│   └── utils/                # Tauri invoke API 封装
├── src-tauri/                # Rust 后端
│   └── src/
│       ├── commands/         # Tauri 命令（connection/database/query）
│       ├── state.rs          # 全局状态（连接池管理）
│       └── error.rs          # 错误类型
└── Cargo.toml                # Workspace 配置
```
