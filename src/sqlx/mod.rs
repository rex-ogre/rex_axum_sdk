use serde::de::DeserializeOwned;
use sqlx::{
    postgres::{PgQueryResult, PgRow},
    prelude::FromRow,
    Error, PgPool, Postgres,
};
use std::fmt::Debug;
use tracing::{info, instrument};

#[async_trait::async_trait]
pub trait PgPoolExt {
    async fn execute<'a, T>(&self, query: &'a str, params: T) -> Result<PgQueryResult, Error>
    where
        T: Send + Sync + IntoIterator + 'a,
        T::Item: 'a + Send + Sync + sqlx::Encode<'a, Postgres> + sqlx::Type<Postgres>;

    async fn fetch<T>(
        &self,
        query: &str,
        params: Vec<Box<dyn PostgresParam>>,
    ) -> Result<Vec<T>, Error>
    where
        T: for<'r> FromRow<'r, PgRow> + DeserializeOwned + Send + Unpin;
}

#[async_trait::async_trait]
impl PgPoolExt for PgPool {
    #[instrument(skip(self, query, params), fields(query = %query))]
    async fn execute<'a, T>(&self, query: &'a str, params: T) -> Result<PgQueryResult, Error>
    where
        T: Send + Sync + IntoIterator + 'a,
        T::Item: 'a + Send + Sync + sqlx::Encode<'a, Postgres> + sqlx::Type<Postgres>,
    {
        let mut q = sqlx::query(query);
        for param in params {
            q = q.bind(param);
        }
        let result = q.execute(self).await;
        match &result {
            Ok(pg_result) => info!("Query執行成功，影響了 {} 行", pg_result.rows_affected()),
            Err(e) => info!("Query執行失敗：{:?}", e),
        }
        result
    }

    #[instrument(skip(self, params), fields(query = %query))]
    async fn fetch<T>(
        &self,
        query: &str,
        params: Vec<Box<dyn PostgresParam>>,
    ) -> Result<Vec<T>, Error>
    where
        T: for<'r> FromRow<'r, PgRow> + DeserializeOwned + Send + Unpin,
    {
        info!("執行查詢: {}", query);
        let param_count = params.len();

        let mut sqlx_query = sqlx::query(query);
        for param in params.iter() {
            sqlx_query = param.bind_to_query(sqlx_query);
        }

        info!("綁定了 {} 個參數", param_count);

        let rows = sqlx_query.fetch_all(self).await?;
        rows.into_iter()
            .map(|row| T::from_row(&row))
            .collect::<Result<Vec<_>, _>>()
    }
}

// 參數特徵定義，添加 Debug trait
pub trait PostgresParam: Send + Debug {
    fn bind_to_query<'q>(
        &'q self,
        query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>;
}

// 為基本類型實現 PostgresParam
impl<T> PostgresParam for T
where
    T: 'static + Send + Sync + Debug + for<'q> sqlx::Encode<'q, Postgres> + sqlx::Type<Postgres>,
{
    fn bind_to_query<'q>(
        &'q self,
        query: sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments>,
    ) -> sqlx::query::Query<'q, Postgres, sqlx::postgres::PgArguments> {
        query.bind(self)
    }
}

// Query Builder trait
pub trait QueryBuilder {
    fn build_query(&self) -> String;
    fn build_params(&self) -> Vec<Box<dyn PostgresParam>>;
}

pub trait QueryContext: Send + Debug {
    fn get_email(&self) -> Option<String> {
        None
    }
    fn get_name(&self) -> Option<String> {
        None
    }
    fn get_tenant_id(&self) -> Option<String> {
        None
    }
    fn get_user_id(&self) -> Option<String> {
        None
    }
    fn into_params(self) -> Vec<Box<dyn PostgresParam>>;
}
#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};
    use sqlx::FromRow;

    #[derive(Debug, FromRow, Serialize, Deserialize)]
    struct TestUser {
        id: i32,
        name: String,
        email: String,
    }

    async fn setup_test_db() -> PgPool {
        let database_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgres://Rex@localhost:5432/mydb".to_string());

        PgPool::connect(&database_url)
            .await
            .expect("無法連接到測試數據庫")
    }

    #[tokio::test]
    async fn test_execute_query() {
        let pool = setup_test_db().await;

        // 創建測試表
        let create_table = "CREATE TEMPORARY TABLE IF NOT EXISTS test_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )";

        let result = pool.execute(create_table, Vec::<String>::new()).await;
        assert!(result.is_ok(), "無法創建測試表");
    }

    #[tokio::test]
    async fn test_fetch_query() {
        let pool = setup_test_db().await;

        // 分步驟執行：先建表
        let create_table = "CREATE TABLE IF NOT EXISTS test_users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL
        )";
        pool.execute(create_table, Vec::<String>::new())
            .await
            .expect("無法創建測試表");

        // 再插入數據
        let insert_data = "INSERT INTO test_users (name, email) VALUES ($1, $2)";
        pool.execute(insert_data, vec!["test_user", "test@example.com"])
            .await
            .expect("無法插入測試數據");

        // 測試查詢
        let query = "SELECT * FROM test_users WHERE name = $1";
        let params: Vec<Box<dyn PostgresParam>> = vec![Box::new("test_user".to_string())];

        let results: Vec<TestUser> = pool.fetch(query, params).await.expect("查詢失敗");

        assert!(!results.is_empty(), "應該返回至少一個結果");
        assert_eq!(results[0].name, "test_user");
        assert_eq!(results[0].email, "test@example.com");
    }

    #[tokio::test]
    async fn test_query_builder() {
        let query = TestUserQuery {
            name: "test_user".to_string(),
        };

        assert_eq!(query.build_query(), "SELECT * FROM users WHERE name = $1");

        let params = query.build_params();
        assert_eq!(params.len(), 1);
    }

    // 測試用的查詢構建器
    #[derive(Debug)]
    struct TestUserQuery {
        name: String,
    }

    impl QueryBuilder for TestUserQuery {
        fn build_query(&self) -> String {
            "SELECT * FROM users WHERE name = $1".to_string()
        }

        fn build_params(&self) -> Vec<Box<dyn PostgresParam>> {
            vec![Box::new(self.name.clone())]
        }
    }

    #[tokio::test]
    async fn test_query_context() {
        let context = TestContext {
            email: "test@example.com".to_string(),
            name: "test_user".to_string(),
        };

        assert_eq!(context.get_email(), Some("test@example.com".to_string()));
        assert_eq!(context.get_name(), Some("test_user".to_string()));

        let params = context.into_params();
        assert_eq!(params.len(), 2);
    }

    // 測試用的上下文實現
    #[derive(Debug)]
    struct TestContext {
        email: String,
        name: String,
    }

    impl QueryContext for TestContext {
        fn get_email(&self) -> Option<String> {
            Some(self.email.clone())
        }

        fn get_name(&self) -> Option<String> {
            Some(self.name.clone())
        }

        fn into_params(self) -> Vec<Box<dyn PostgresParam>> {
            vec![Box::new(self.email), Box::new(self.name)]
        }
    }

    #[tokio::test]
    async fn test_postgres_param_implementation() {
        let param: Box<dyn PostgresParam> = Box::new("test_value".to_string());
        let query = sqlx::query("SELECT $1");
        let _bound_query = param.bind_to_query(query);

        assert!(true, "Parameter binding should compile successfully");
    }
}
