use async_trait::async_trait;
use sea_orm::{EntityTrait, Select};
use std::fmt::Debug;

/// 上下文信息提供者 Trait (开发者需要实现此接口)
pub trait ContextInfoProvider: Send + Sync + Debug {
    /// 获取当前用户ID
    fn get_current_user_id(&self) -> Option<String>;

    /// 获取当前用户名
    fn get_current_user_name(&self) -> Option<String>;
    
    /// 获取当前用户真实姓名
    fn get_current_real_name(&self) -> Option<String>;

    /// 获取当前租户ID
    fn get_current_tenant_id(&self) -> Option<String>;

    /// 获取当前租户名称
    fn get_current_tenant_name(&self) -> Option<String>;
}

/// 查询扩展 Trait (用于查询数据库记录)
pub trait QueryExtensions: EntityTrait {
    /// 查询未删除的记录
    fn find_not_deleted() -> Select<Self>;

    /// 按租户ID查询 (查找属于某个租户的记录)
    fn find_by_tenant_id(tenant_id: &str) -> Select<Self>;

    /// 按创建人ID查询 (查找某个用户创建的记录)
    fn find_by_creator_id(user_id: &str) -> Select<Self>;

    /// 按创建人名查询 (查找某个用户创建的记录)
    fn find_by_creator_name(user_name: &str) -> Select<Self>;
}

/// 软删除扩展 Trait
#[async_trait]
pub trait SoftDeleteExt: EntityTrait {
    /// 软删除单个记录
    async fn soft_delete<C>(db: &C, id: &str) -> Result<(), sea_orm::DbErr>
    where
        C: sea_orm::ConnectionTrait;

    /// 软删除多个记录
    async fn soft_delete_many<C>(db: &C, ids: &[String]) -> Result<(), sea_orm::DbErr>
    where
        C: sea_orm::ConnectionTrait;
}

/// 自动字段上下文结构
#[derive(Debug, Clone, Default)]
pub struct AutoFieldContext {
    pub user_id: Option<String>,
    pub user_name: Option<String>,
    pub real_name: Option<String>,
    pub tenant_id: Option<String>,
    pub tenant_name: Option<String>,
}

impl AutoFieldContext {
    /// 设置用户信息
    pub fn with_user(mut self, user_id: Option<String>, user_name: Option<String>, real_name: Option<String>) -> Self {
        self.user_id = user_id;
        self.user_name = user_name;
        self.real_name = real_name;
        self
    }

    /// 设置租户信息
    pub fn with_tenant(mut self, tenant_id: Option<String>, tenant_name: Option<String>) -> Self {
        self.tenant_id = tenant_id;
        self.tenant_name = tenant_name;
        self
    }

    /// 从上下文提供者创建上下文
    pub fn from_provider(provider: &dyn ContextInfoProvider) -> Self {
        Self {
            user_id: provider.get_current_user_id(),
            user_name: provider.get_current_user_name(),
            real_name: provider.get_current_real_name(),
            tenant_id: provider.get_current_tenant_id(),
            tenant_name: provider.get_current_tenant_name(),
        }
    }

    /// 获取当前上下文
    /// 调用全局注册的上下文获取函数
    pub fn current_safe() -> Self {
        get_current_context()
    }
}

/// 默认的上下文获取函数（返回空上下文）
fn default_context_getter() -> AutoFieldContext {
    AutoFieldContext::default()
}

/// 全局上下文获取函数指针（默认指向空实现）
static mut CONTEXT_GETTER: fn() -> AutoFieldContext = default_context_getter;

/// 注册上下文获取函数
/// 用户提供一个函数来获取当前请求的上下文信息
pub fn register_context_getter(getter: fn() -> AutoFieldContext) {
    unsafe {
        CONTEXT_GETTER = getter;
    }
}

/// 获取当前上下文的函数
/// 直接调用用户注册的获取函数
pub fn get_current_context() -> AutoFieldContext {
    unsafe {
        CONTEXT_GETTER()
    }
}
