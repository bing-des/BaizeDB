# BaizeDB 项目 - 长期记忆

## 项目概况
- **路径**: c:/codes/BaizeDB
- **类型**: PC端数据库管理工具（类 TablePlus/DBeaver）
- **技术栈**: Tauri v2 + Rust 后端 + React 18 + TypeScript + Tailwind CSS v3
- **数据库支持**: MySQL (sqlx 0.8) + PostgreSQL (sqlx 0.8) + Redis (redis 0.27)

## 架构决策
- **后端**: Rust workspace，src-tauri 为唯一成员，sqlx 异步驱动 + redis aio
- **前端**: React 18 + Zustand 5 + react-resizable-panels 布局
- **SQL编辑器**: @uiw/react-codemirror + @codemirror/lang-sql
- **连接持久化**: Zustand persist 存 localStorage（密码明文，后续可加密）
- **主题**: Tailwind darkMode="class" + CSS变量
- **PG 多库**: 使用 db_pools（key="connection_id:database"），按需创建独立连接池
- **Redis 连接**: MultiplexedConnection（支持 Clone），clone 出来给每个命令用

## 已实现功能（v0.1.0）
1. 连接管理 - 新建/测试/删除，MySQL/PG/Redis 自动默认端口
2. 数据库树 - MySQL: 库→表；PG: 库→Schema→表；Redis: db0~db15→Key列表
3. SQL编辑器 - 语法高亮、Ctrl+Enter执行、结果表格、CSV导出
4. 表格查看器 - 分页200行、列结构面板
5. Redis Key 查看器 - string/hash/list/set/zset 可视化
6. 多标签页
7. 亮/暗/系统主题

## 编译状态
- Rust: `cargo check` 零 error（2026-04-09）
- TypeScript: `tsc --noEmit` 零 error（2026-04-09）

## 启动命令
```bash
npm install && npm run tauri dev
```
