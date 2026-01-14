# auto_field_trait

## 项目概述

`auto_field_trait` 是一个基于 Rust 语言开发的自动字段处理库，专为 SeaORM 框架设计，提供了一系列自动处理数据库字段的功能，简化了开发者在数据库操作中的重复工作。

### 功能特性

- **自动ID生成**：支持基于雪花算法的ID自动生成
- **时间戳管理**：自动填充创建时间和更新时间
- **审计日志**：自动记录创建人和更新人信息
- **租户支持**：自动处理多租户场景下的租户ID和租户名称
- **版本控制**：自动管理数据版本号，支持乐观锁
- **软删除**：支持数据软删除，保留历史记录
- **查询扩展**：提供便捷的查询方法，如按租户ID查询、按创建人查询等
- **批量操作**：支持批量插入和批量更新，自动处理字段填充

### 技术架构

- **语言**：Rust
- **核心依赖**：
  - `sea-orm`：数据库ORM框架
  - `spring`：Rust应用框架
  - `anyhow`：错误处理库
  - `async-trait`：异步trait支持

## 安装与配置

### 安装

在 `Cargo.toml` 文件中添加依赖：

```toml
dependencies =
    auto_field_trait = { version = "0.1.3", git = "https://github.com/tttq/auto_field_trait.git", features = ["postgres", "with-web"] }
    sea-orm = "0.12"
    spring = "0.1"
```

### 配置

1. **数据库配置**：

在配置文件（如 `config.toml`）中添加数据库连接配置：

```toml
[database]
uri = "mysql://root:password@localhost:3306/test_db"
max_connections = 10
min_connections = 1
enable_logging = true
connect_timeout = 5000
idle_timeout = 3600000
enable_soft_delete = true
enable_tenant_filter = true
skip_table = ["system_config"]
```

2. **注册插件**：

在应用启动时注册 `HookedSeaOrmPlugin` 插件：

```rust
use auto_field_trait::HookedSeaOrmPlugin;
use spring::app::AppBuilder;

#[tokio::main]
async fn main() {
    let mut app = AppBuilder::new();
    app.add_plugin(HookedSeaOrmPlugin);
    app.run().await.unwrap();
}
```

3. **注册上下文获取器**：

在应用中注册上下文获取器，用于获取当前用户和租户信息：

```rust
use auto_field_trait::register_context_getter;

register_context_getter(|| {
    AutoFieldContext::new(
        Some("user_id".to_string()),
        Some("user_name".to_string()),
        Some("real_name".to_string()),
        Some("tenant_id".to_string()),
        Some("tenant_name".to_string()),
    )
});
```

## 使用指南

### 基本使用

1. **定义实体**：

```rust
use sea_orm::entity::prelude::*;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel)]
#[sea_orm(table_name = "users")]
pub struct Model {
    #[sea_orm(primary_key)]
    pub id: String,
    pub name: String,
    pub email: String,
    pub create_time: Option<DateTime<Utc>>,
    pub update_time: Option<DateTime<Utc>>,
    pub create_by: Option<String>,
    pub create_id: Option<String>,
    pub update_by: Option<String>,
    pub update_id: Option<String>,
    pub tenant_id: Option<String>,
    pub tenant_name: Option<String>,
    pub version: Option<i32>,
    pub delete_flag: Option<i32>,
}
```

2. **实现自动字段**：

使用 `#[derive(AutoField)]` 宏为实体自动生成字段处理逻辑：

```rust
use auto_field_macros::AutoField;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, AutoField)]
#[sea_orm(table_name = "users")]
#[auto_field(snowflake_id, timestamps, audit, tenant, version, soft_delete)]
pub struct Model {
    // ... 字段定义
}
```

3. **使用查询扩展**：

```rust
use auto_field_trait::QueryExtensions;

// 查询未删除的记录
let users = User::find_not_deleted().all(db).await?;

// 按租户ID查询
let users = User::find_by_tenant_id("tenant_123").all(db).await?;

// 按创建人ID查询
let users = User::find_by_creator_id("user_456").all(db).await?;
```

4. **使用软删除**：

```rust
use auto_field_trait::CustomizationExt;

// 软删除单个记录
User::soft_delete(db, "user_789").await?;

// 软删除多个记录
User::soft_delete_many(db, &["user_101", "user_102"]).await?;
```

5. **批量操作**：

```rust
use auto_field_trait::CustomizationExt;

// 批量更新
let update_many = User::batch_update()
    .col_expr(User::Column::Name, Expr::value("new_name"))
    .filter(User::Column::Id.eq("user_123"))
    .exec(db)
    .await?;

// 批量插入
let users = vec![
    UserActiveModel {
        name: ActiveValue::Set("user_1".to_string()),
        email: ActiveValue::Set("user_1@example.com".to_string()),
        ..Default::default()
    },
    UserActiveModel {
        name: ActiveValue::Set("user_2".to_string()),
        email: ActiveValue::Set("user_2@example.com".to_string()),
        ..Default::default()
    },
];

let insert_result = User::batch_insert_many(users)
    .exec(db)
    .await?;
```

## 注意事项

### 环境要求

- **Rust版本**：1.65.0 及以上
- **SeaORM版本**：0.12.x
- **spring版本**：0.1.x

### 限制条件

1. 目前仅支持 SeaORM 框架
2. 仅支持 MySQL 和 PostgreSQL 数据库
3. 雪花算法ID生成依赖 spring 框架的 SnowflakeIdGenerator 组件
4. 上下文获取器需要手动注册，否则将使用默认空实现

### 常见问题

1. **问题**：自动字段没有被正确填充
   **解决方案**：确保已正确注册 `HookedSeaOrmPlugin` 插件和上下文获取器

2. **问题**：软删除功能不起作用
   **解决方案**：确保在实体上添加了 `#[auto_field(soft_delete)]` 标记，并在配置中启用了软删除

3. **问题**：批量插入失败
   **解决方案**：确保所有必填字段都已正确设置，并且数据库连接配置正确

## 项目目录结构

```
auto_field_trait/
├── src/
│   ├── auto_field_trait.rs    # 核心trait定义
│   ├── config.rs              # 配置定义
│   ├── extract_hook.rs        # 提取钩子实现
│   ├── lib.rs                 # 库入口文件
│   └── pagination.rs          # 分页功能实现
├── Cargo.toml                 # 依赖配置
└── README.md                  # 项目文档
```

### 文件用途说明

| 文件/文件夹 | 用途 |
| --- | --- |
| `src/auto_field_trait.rs` | 定义核心trait，如ContextInfoProvider、QueryExtensions、CustomizationExt等 |
| `src/config.rs` | 定义数据库配置结构SeaOrmConfig |
| `src/extract_hook.rs` | 实现数据库连接钩子，用于初始化HookedConnection |
| `src/lib.rs` | 库的入口文件，导出核心功能和类型 |
| `src/pagination.rs` | 实现分页功能，提供Page、PageResult、Pagination等类型 |
| `Cargo.toml` | 项目依赖和构建配置 |
| `README.md` | 项目文档，包含使用说明和API参考 |