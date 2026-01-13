use parking_lot::RwLock;
use sea_orm::{ConnectionTrait, DatabaseBackend, DbErr, ExecResult, QueryResult, Statement};
use sqlparser::dialect::GenericDialect;
use sqlparser::parser::Parser;
use std::collections::HashSet;
use std::sync::Arc;

/// 查询钩子 Trait，用于拦截和修改 SQL 查询
pub trait QueryHook: Send + Sync {
    /// 在执行查询前调用，可以修改 SQL 语句
    fn before_query(&self, sql: &str) -> Result<Option<String>, DbErr>;

    /// 在执行查询后调用
    fn after_query(&self, sql: &str, result: &Result<(), &DbErr>);
}

/// 默认查询钩子实现
#[derive(Clone)]
pub struct DefaultQueryHook {
    /// 是否启用软删除过滤
    pub enable_soft_delete: bool,

    /// 是否启用租户过滤
    pub enable_tenant_filter: bool,

    /// 需要跳过默认过滤的表名集合
    skip_tables: Arc<RwLock<HashSet<String>>>,
}

impl DefaultQueryHook {
    /// 创建新的默认查询钩子
    pub fn new(enable_soft_delete :bool, enable_tenant_filter:bool) -> Self {
        Self {
            enable_soft_delete,
            enable_tenant_filter,
            skip_tables: Arc::new(RwLock::new(HashSet::new())),
        }
    }

    /// 添加需要跳过默认过滤的表名
    pub fn add_skip_table(&self, table_name: &str) {
        let mut skip_tables = self.skip_tables.write();
        skip_tables.insert(table_name.to_lowercase());
    }

    /// 移除需要跳过默认过滤的表名
    pub fn remove_skip_table(&self, table_name: &str) {
        let mut skip_tables = self.skip_tables.write();
        skip_tables.remove(&table_name.to_lowercase());
    }

    /// 检查表是否需要跳过默认过滤
    fn should_skip_table(&self, table_name: &str) -> bool {
        let skip_tables = self.skip_tables.read();
        skip_tables.contains(&table_name.to_lowercase())
    }

    /// 解析 SQL 并添加默认查询条件
    fn add_default_conditions(&self, sql: &str) -> Result<String, DbErr> {
        let dialect = GenericDialect {};

        match Parser::parse_sql(&dialect, sql) {
            Ok(mut statements) => {
                if statements.is_empty() {
                    return Ok(sql.to_string());
                }

                let statement = &mut statements[0];

                // 只处理 SELECT 语句
                if let sqlparser::ast::Statement::Query(query) = statement {
                    if let Some(table_name) = self.extract_table_name(query) {
                        // 检查是否需要跳过该表的默认过滤
                        if self.should_skip_table(&table_name) {
                            return Ok(sql.to_string());
                        }

                        // 添加默认查询条件
                        self.add_conditions_to_query(query, &table_name)?;
                    }
                }

                Ok(statements[0].to_string())
            }
            Err(e) => {
                log::warn!("Failed to parse SQL: {}, error: {}", sql, e);
                // 解析失败时返回原始 SQL
                Ok(sql.to_string())
            }
        }
    }

    /// 从查询中提取表名，支持嵌套查询
    fn extract_table_name(&self, query: &sqlparser::ast::Query) -> Option<String> {
        self.extract_table_name_from_set_expr(&query.body)
    }
    
    /// 从SetExpr中提取表名，支持递归处理嵌套查询
    fn extract_table_name_from_set_expr(&self, set_expr: &sqlparser::ast::SetExpr) -> Option<String> {
        match set_expr {
            sqlparser::ast::SetExpr::Select(select) => {
                // 检查是否有FROM子句
                if !select.from.is_empty() {
                    // 从第一个表中提取表名
                    if let sqlparser::ast::TableFactor::Table { name, .. } = &select.from[0].relation {
                        if let Some(last_ident) = name.0.last() {
                            let mut table_name = last_ident.to_string().to_lowercase();
                            // 移除可能存在的引号
                            if table_name.starts_with('"') && table_name.ends_with('"') {
                                table_name = table_name[1..table_name.len()-1].to_string();
                            }
                            if !table_name.is_empty() {
                                return Some(table_name);
                            }
                        }
                    }
                    // 检查是否是子查询
                    else if let sqlparser::ast::TableFactor::Derived { subquery, .. } = &select.from[0].relation {
                        // 递归处理子查询
                        return self.extract_table_name(subquery);
                    }
                }
                None
            }
            // 处理其他类型的SetExpr，如子查询
            sqlparser::ast::SetExpr::Query(query) => {
                // 递归处理子查询
                self.extract_table_name(query)
            }
            _ => None
        }
    }

    /// 向查询中添加默认条件
    fn add_conditions_to_query(
        &self,
        query: &mut sqlparser::ast::Query,
        _table_name: &str,
    ) -> Result<(), DbErr> {
        // 处理 COUNT 查询，将条件添加到内部子查询
        if let sqlparser::ast::SetExpr::Select(select) = &mut *query.body {
            // 检查是否是 COUNT 查询（SELECT COUNT(*) FROM ...）
            if self.is_count_query(select) {
                // 遍历 FROM 子句，查找子查询
                for table in &mut select.from {
                    if let sqlparser::ast::TableFactor::Derived { subquery, .. } = &mut table.relation {
                        // 向内部子查询添加条件
                        self.add_conditions_to_query(subquery, "")?;
                    }
                }
                return Ok(());
            }
        }
        
        // 非 COUNT 查询，直接向查询体添加条件
        self.add_conditions_to_set_expr(&mut query.body)
    }
    
    /// 检查是否是 COUNT 查询
    fn is_count_query(&self, select: &sqlparser::ast::Select) -> bool {
        // 检查 SELECT 列表是否只有 COUNT(*)
        if select.projection.len() != 1 {
            return false;
        }
        
        match &select.projection[0] {
            sqlparser::ast::SelectItem::ExprWithAlias { expr, .. } | sqlparser::ast::SelectItem::UnnamedExpr(expr) => {
                if let sqlparser::ast::Expr::Function(func) = expr {
                    // 检查函数名是否为 COUNT
                    if func.name.0.last().map_or(false, |ident| ident.to_string().eq_ignore_ascii_case("COUNT")) {
                        // 检查是否包含 COUNT(*) 或 COUNT(1)
                        // 简化检查，不依赖 FunctionArguments 的内部结构
                        let func_str = func.to_string();
                        return func_str.eq_ignore_ascii_case("COUNT(*)") || func_str.eq_ignore_ascii_case("count(*)");
                    }
                }
            }
            _ => {}
        }
        
        false
    }
    
    /// 向SetExpr中添加默认条件，支持递归处理嵌套查询
    fn add_conditions_to_set_expr(
        &self,
        set_expr: &mut sqlparser::ast::SetExpr,
    ) -> Result<(), DbErr> {
        match set_expr {
            sqlparser::ast::SetExpr::Select(select) => {
                self.add_conditions_to_select(select)
            }
            sqlparser::ast::SetExpr::Query(query) => {
                // 递归处理嵌套查询
                self.add_conditions_to_query(query, "")
            }
            _ => Ok(()),
        }
    }
    
    /// 从Select语句中提取表别名（仅提取别名，不提取表名）
    fn extract_table_alias_or_name(&self, select: &sqlparser::ast::Select) -> Option<String> {
        if select.from.is_empty() {
            return None;
        }
        
        let table = &select.from[0];
        match &table.relation {
            sqlparser::ast::TableFactor::Table { alias, .. } => {
                // 仅提取表别名，没有别名时返回None
                if let Some(alias) = alias {
                    return Some(alias.name.to_string());
                }
                None
            },
            _ => None,
        }
    }
    
    /// 创建带表别名的字段表达式
    fn create_field_expr(&self, field_name: &str, table_alias: Option<&str>) -> sqlparser::ast::Expr {
        match table_alias {
            Some(alias) => {
                // 使用表别名.字段名格式
                sqlparser::ast::Expr::CompoundIdentifier(vec![
                    sqlparser::ast::Ident::new(alias),
                    sqlparser::ast::Ident::new(field_name)
                ])
            },
            None => {
                // 直接使用字段名
                sqlparser::ast::Expr::Identifier(sqlparser::ast::Ident::new(field_name))
            }
        }
    }
    
    /// 向Select语句中添加默认条件
    fn add_conditions_to_select(
        &self,
        select: &mut sqlparser::ast::Select,
    ) -> Result<(), DbErr> {
        let mut conditions = Vec::new();
        
        // 提取表别名或表名
        let table_alias = self.extract_table_alias_or_name(select);
        let table_alias_ref = table_alias.as_deref();

        // 添加软删除过滤条件
        if self.enable_soft_delete {
            conditions.push(sqlparser::ast::Expr::BinaryOp {
                left: Box::new(self.create_field_expr("delete_flag", table_alias_ref)),
                op: sqlparser::ast::BinaryOperator::Eq,
                right: Box::new(sqlparser::ast::Expr::Value(sqlparser::ast::Value::Number("0".to_string(), false).with_empty_span())),
            });
        }

        // 添加租户过滤条件
        if self.enable_tenant_filter {
            let context = crate::auto_field_trait::AutoFieldContext::current_safe();
            if let Some(tenant_id) = context.tenant_id {
                if !tenant_id.is_empty() {
                    conditions.push(sqlparser::ast::Expr::BinaryOp {
                        left: Box::new(self.create_field_expr("tenant_id", table_alias_ref)),
                        op: sqlparser::ast::BinaryOperator::Eq,
                        right: Box::new(sqlparser::ast::Expr::Value(sqlparser::ast::Value::SingleQuotedString(tenant_id).with_empty_span())),
                    });
                }
            }
        }

        // 将条件合并并添加到查询中
        if !conditions.is_empty() {
            let combined_condition = if conditions.len() == 1 {
                conditions.into_iter().next().unwrap()
            } else {
                sqlparser::ast::Expr::Nested(Box::new(conditions.into_iter().reduce(|left, right| {
                    sqlparser::ast::Expr::BinaryOp {
                        left: Box::new(left),
                        op: sqlparser::ast::BinaryOperator::And,
                        right: Box::new(right),
                    }
                }).unwrap()))
            };

            // 添加到 WHERE 子句
            if let Some(ref mut selection) = select.selection {
                *selection = sqlparser::ast::Expr::BinaryOp {
                    left: Box::new(std::mem::replace(selection, combined_condition.clone())),
                    op: sqlparser::ast::BinaryOperator::And,
                    right: Box::new(combined_condition),
                };
            } else {
                select.selection = Some(combined_condition);
            }
        }
        
        Ok(())
    }
}

impl QueryHook for DefaultQueryHook {
    fn before_query(&self, sql: &str) -> Result<Option<String>, DbErr> {
        // 只处理 SELECT 语句
        let sql_upper = sql.trim().to_uppercase();
        if !sql_upper.starts_with("SELECT") {
            return Ok(None);
        }

        // 解析并添加默认条件
        match self.add_default_conditions(sql) {
            Ok(modified_sql) => {
                log::info!("Modified SQL: {}", modified_sql);
                if modified_sql != sql {
                    return Ok(Some(modified_sql));
                }
            }
            Err(e) => {
                log::warn!("Failed to add default conditions to SQL: {}, error: {}", sql, e);
            }
        }

        Ok(None)
    }

    fn after_query(&self, _sql: &str, _result: &Result<(), &DbErr>) {
        // 移除调试日志，提高性能
    }
}

/// 带有查询钩子的连接包装器
pub struct HookedConnection<C> {
    inner: C,
    hook: Arc<dyn QueryHook>,
}

impl<C: Clone> Clone for HookedConnection<C> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            hook: Arc::clone(&self.hook),
        }
    }
}

impl<C> HookedConnection<C> {
    /// 创建新的钩子连接
    pub fn new(inner: C, hook: Arc<dyn QueryHook + 'static>) -> Self {
        Self { inner, hook }
    }
    
    /// 从内部连接和全局钩子创建新的钩子连接
    pub fn new_with_global_hook(inner: C) -> Option<Self> {
        get_extract_hook().map(|hook| Self { inner, hook })
    }
}

#[async_trait::async_trait]
impl<C> ConnectionTrait for HookedConnection<C>
where
    C: ConnectionTrait + Send + Sync + 'static,
{
    fn get_database_backend(&self) -> DatabaseBackend {
        self.inner.get_database_backend()
    }

    async fn execute(&self, stmt: Statement) -> Result<ExecResult, DbErr> {
        let sql = stmt.to_string();
        log::info!("Executing SQL: {}", sql);
        if let Some(modified_sql) = self.hook.before_query(&sql)? {
            log::info!("Modified SQL: {}", modified_sql);
            let modified_stmt = Statement::from_string(
                self.get_database_backend(),
                &modified_sql,
            );
            let result = self.inner.execute(modified_stmt).await;
            self.hook.after_query(&modified_sql, &result.as_ref().map(|_| ()));
            result
        } else {
            let result = self.inner.execute(stmt).await;
            self.hook.after_query(&sql, &result.as_ref().map(|_| ()));
            result
        }
    }

    async fn execute_unprepared(&self, sql: &str) -> Result<ExecResult, DbErr> {
        log::info!("Executing unprepared SQL: {}", sql);
        if let Some(modified_sql) = self.hook.before_query(sql)? {
            log::info!("Modified SQL: {}", modified_sql);
            let result = self.inner.execute_unprepared(&modified_sql).await;
            self.hook.after_query(&modified_sql, &result.as_ref().map(|_| ()));
            result
        } else {
            let result = self.inner.execute_unprepared(sql).await;
            self.hook.after_query(sql, &result.as_ref().map(|_| ()));
            result
        }
    }

    async fn query_one(&self, stmt: Statement) -> Result<Option<QueryResult>, DbErr> {
        let sql = stmt.to_string();
        if let Some(modified_sql) = self.hook.before_query(&sql)? {
            let modified_stmt = Statement::from_string(
                self.get_database_backend(),
                &modified_sql,
            );
            let result = self.inner.query_one(modified_stmt).await;
            self.hook.after_query(&modified_sql, &result.as_ref().map(|_| ()));
            result
        } else {
            let result = self.inner.query_one(stmt).await;
            self.hook.after_query(&sql, &result.as_ref().map(|_| ()));
            result
        }
    }

    async fn query_all(&self, stmt: Statement) -> Result<Vec<QueryResult>, DbErr> {
        let sql = stmt.to_string();
        if let Some(modified_sql) = self.hook.before_query(&sql)? {
            let modified_stmt = Statement::from_string(
                self.get_database_backend(),
                &modified_sql,
            );
            let result = self.inner.query_all(modified_stmt).await;
            self.hook.after_query(&modified_sql, &result.as_ref().map(|_| ()));
            result
        } else {
            let result = self.inner.query_all(stmt).await;
            self.hook.after_query(&sql, &result.as_ref().map(|_| ()));
            result
        }
    }

    fn support_returning(&self) -> bool {
        self.inner.support_returning()
    }

    fn is_mock_connection(&self) -> bool {
        self.inner.is_mock_connection()
    }
}

/// 全局查询钩子注册表
static EXTRACT_HOOK_REGISTRY: parking_lot::RwLock<Option<Arc<dyn QueryHook>>> = parking_lot::RwLock::new(None);

/// 注册全局查询钩子
pub fn register_extract_hook(hook: Arc<dyn QueryHook>) {
    let mut registry = EXTRACT_HOOK_REGISTRY.write();
    *registry = Some(hook);
}

/// 获取全局查询钩子
pub fn get_extract_hook() -> Option<Arc<dyn QueryHook>> {
    let registry = EXTRACT_HOOK_REGISTRY.read();
    registry.clone()
}

/// 移除全局查询钩子
pub fn unregister_extract_hook() {
    let mut registry = EXTRACT_HOOK_REGISTRY.write();
    *registry = None;
}