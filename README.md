# auto_field_trait

## Project Overview

`auto_field_trait` is an automatic field processing library developed in Rust, designed specifically for the SeaORM framework. It provides a series of functions to automatically handle database fields, simplifying repetitive work for developers in database operations.

### Features

- **Automatic ID Generation**: Support for automatic ID generation based on snowflake algorithm
- **Timestamp Management**: Automatic filling of creation time and update time
- **Audit Logging**: Automatic recording of creator and updater information
- **Tenant Support**: Automatic handling of tenant ID and tenant name in multi-tenant scenarios
- **Version Control**: Automatic management of data version numbers, supporting optimistic locking
- **Soft Delete**: Support for soft deletion of data, preserving historical records
- **Query Extensions**: Provide convenient query methods, such as query by tenant ID, query by creator, etc.
- **Batch Operations**: Support for batch insertion and batch update, with automatic field filling

### Technical Architecture

- **Language**: Rust
- **Core Dependencies**:
  - `sea-orm`: Database ORM framework
  - `spring`: Rust application framework
  - `anyhow`: Error handling library
  - `async-trait`: Asynchronous trait support

## Installation and Configuration

### Installation

Add dependencies to your `Cargo.toml` file:

```toml
dependencies =
    auto_field_trait = { version = "0.1.3", git = "https://github.com/tttq/auto_field_trait.git", features = ["postgres", "with-web"] }
    sea-orm = "0.12"
    spring = "0.1"
```

### Configuration

1. **Database Configuration**:

Add database connection configuration to your configuration file (e.g., `config.toml`):

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

2. **Register Plugin**:

Register the `HookedSeaOrmPlugin` plugin when starting your application:

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

3. **Register Context Getter**:

Register a context getter in your application to obtain current user and tenant information:

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

## Usage Guide

### Basic Usage

1. **Define Entity**:

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

2. **Implement Auto Fields**:

Use the `#[derive(AutoField)]` macro to automatically generate field processing logic for your entity:

```rust
use auto_field_macros::AutoField;

#[derive(Clone, Debug, PartialEq, DeriveEntityModel, AutoField)]
#[sea_orm(table_name = "users")]
#[auto_field(snowflake_id, timestamps, audit, tenant, version, soft_delete)]
pub struct Model {
    // ... field definitions
}
```

3. **Use Query Extensions**:

```rust
use auto_field_trait::QueryExtensions;

// Query not deleted records
let users = User::find_not_deleted().all(db).await?;

// Query by tenant ID
let users = User::find_by_tenant_id("tenant_123").all(db).await?;

// Query by creator ID
let users = User::find_by_creator_id("user_456").all(db).await?;
```

4. **Use Soft Delete**:

```rust
use auto_field_trait::CustomizationExt;

// Soft delete a single record
User::soft_delete(db, "user_789").await?;

// Soft delete multiple records
User::soft_delete_many(db, &["user_101", "user_102"]).await?;
```

5. **Batch Operations**:

```rust
use auto_field_trait::CustomizationExt;

// Batch update
let update_many = User::batch_update()
    .col_expr(User::Column::Name, Expr::value("new_name"))
    .filter(User::Column::Id.eq("user_123"))
    .exec(db)
    .await?;

// Batch insert
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

## Notes

### Environment Requirements

- **Rust Version**: 1.65.0 or higher
- **SeaORM Version**: 0.12.x
- **Spring Version**: 0.1.x

### Limitations

1. Currently only supports SeaORM framework
2. Only supports MySQL and PostgreSQL databases
3. Snowflake algorithm ID generation depends on spring framework's SnowflakeIdGenerator component
4. Context getter needs to be manually registered, otherwise a default empty implementation will be used

### Common Issues

1. **Issue**: Auto fields are not being filled correctly
   **Solution**: Ensure that the `HookedSeaOrmPlugin` plugin and context getter are correctly registered

2. **Issue**: Soft delete functionality doesn't work
   **Solution**: Ensure that the `#[auto_field(soft_delete)]` tag is added to the entity and soft delete is enabled in the configuration

3. **Issue**: Batch insertion fails
   **Solution**: Ensure that all required fields are correctly set and database connection configuration is correct

## Project Directory Structure

```
auto_field_trait/
├── src/
│   ├── auto_field_trait.rs    # Core trait definitions
│   ├── config.rs              # Configuration definitions
│   ├── extract_hook.rs        # Extract hook implementation
│   ├── lib.rs                 # Library entry file
│   └── pagination.rs          # Pagination implementation
├── Cargo.toml                 # Dependency configuration
└── README.md                  # Project documentation
```

### File Usage Description

| File/Folder | Purpose |
| --- | --- |
| `src/auto_field_trait.rs` | Defines core traits such as ContextInfoProvider, QueryExtensions, CustomizationExt, etc. |
| `src/config.rs` | Defines database configuration structure SeaOrmConfig |
| `src/extract_hook.rs` | Implements database connection hooks for initializing HookedConnection |
| `src/lib.rs` | Library entry point, exporting core functions and types |
| `src/pagination.rs` | Implements pagination functionality, providing Page, PageResult, Pagination types |
| `Cargo.toml` | Project dependencies and build configuration |
| `README.md` | Project documentation, including usage instructions and API reference |