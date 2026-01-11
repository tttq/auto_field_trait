/// 自动字段处理库
pub mod auto_field_trait;
pub mod extract_hook;
pub mod config;
pub mod pagination;

use anyhow::Context;
use config::SeaOrmConfig;
use extract_hook::{register_extract_hook, DefaultQueryHook, HookedConnection};
use sea_orm::{ConnectOptions, Database};
use spring::async_trait;
use spring::config::ConfigRegistry;
use spring::plugin::MutableComponentRegistry;
use spring::{app::AppBuilder, error::Result, plugin::Plugin};
use std::sync::Arc;
use std::time::Duration;

/// 数据库连接类型别名
pub type DbConn = HookedConnection<sea_orm::DbConn>;

/// 数据库连接钩子插件，用于初始化并包装数据库连接为HookedConnection
pub struct HookedSeaOrmPlugin;

#[async_trait]
impl Plugin for HookedSeaOrmPlugin {
    async fn build(&self, app: &mut AppBuilder) {
        let config = app
            .get_config::<SeaOrmConfig>()
            .expect("sea-orm plugin config load failed");

        // 创建原始的数据库连接
        let conn = Self::connect(&config)
            .await
            .expect("sea-orm plugin load failed");
        
        // 创建并注册默认查询钩子
        let default_hook = Arc::new(DefaultQueryHook::new());
        register_extract_hook(default_hook.clone());
        
        // 将原始连接包装为HookedConnection
        let hooked_conn = HookedConnection::new(conn.clone(), default_hook);
        
        // 同时注册原始连接和HookedConnection到组件注册表中
        app.add_component(conn)
            .add_component(hooked_conn);
    }
}

impl HookedSeaOrmPlugin {
    /// 连接数据库
    pub async fn connect(config: &SeaOrmConfig) -> Result<sea_orm::DbConn> {
        let mut opt = ConnectOptions::new(&config.uri);
        opt.max_connections(config.max_connections)
            .min_connections(config.min_connections)
            .sqlx_logging(config.enable_logging);

        if let Some(connect_timeout) = config.connect_timeout {
            opt.connect_timeout(Duration::from_millis(connect_timeout));
        }
        if let Some(idle_timeout) = config.idle_timeout {
            opt.idle_timeout(Duration::from_millis(idle_timeout));
        }
        if let Some(acquire_timeout) = config.acquire_timeout {
            opt.acquire_timeout(Duration::from_millis(acquire_timeout));
        }

        Ok(Database::connect(opt)
            .await
            .with_context(|| format!("sea-orm connection failed:{}", &config.uri))?)
    }


}


// 重新导出核心类型和宏，方便用户使用
pub use auto_field_trait::{register_context_getter, AutoFieldContext, ContextInfoProvider, QueryExtensions, CustomizationExt};
pub use pagination::{Page, PageResult, Pagination, PaginationExt};
