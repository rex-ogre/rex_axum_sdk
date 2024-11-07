use rex_axum_sdk::sqlx::{PgPoolExt, PostgresParam, QueryBuilder, QueryContext};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use std::fmt::Debug;
use tracing::info;

// 數據模型
#[derive(Debug, FromRow, Serialize, Deserialize)]
struct User {
    id: i32,
    name: String,
    email: String,
    tenant_id: String,
}

#[derive(Debug, FromRow, Serialize, Deserialize)]
struct Product {
    id: i32,
    name: String,
    price: f64,
    tenant_id: String,
}

// 認證上下文
#[derive(Debug)]
struct AuthContext {
    user_id: String,
    email: String,
    tenant_id: String,
}

impl QueryContext for AuthContext {
    fn get_email(&self) -> Option<String> {
        Some(self.email.clone())
    }

    fn get_user_id(&self) -> Option<String> {
        Some(self.user_id.clone())
    }

    fn get_tenant_id(&self) -> Option<String> {
        Some(self.tenant_id.clone())
    }

    fn into_params(self) -> Vec<Box<dyn PostgresParam>> {
        vec![
            Box::new(self.tenant_id),
            Box::new(self.user_id),
            Box::new(self.email),
        ]
    }
}

// 查詢構建器示例
#[derive(Debug)]
struct UserQuery {
    name_filter: Option<String>,
    email_filter: Option<String>,
    tenant_id: String,
}

impl QueryBuilder for UserQuery {
    fn build_query(&self) -> String {
        let mut conditions = vec!["tenant_id = $1".to_string()];
        let mut param_count = 1;

        if let Some(_) = self.name_filter {
            param_count += 1;
            conditions.push(format!("name = ${}", param_count));
        }

        if let Some(_) = self.email_filter {
            param_count += 1;
            conditions.push(format!("email = ${}", param_count));
        }

        format!("SELECT * FROM users WHERE {}", conditions.join(" AND "))
    }

    fn build_params(&self) -> Vec<Box<dyn PostgresParam>> {
        let mut params: Vec<Box<dyn PostgresParam>> = vec![Box::new(self.tenant_id.clone())];

        if let Some(name) = &self.name_filter {
            params.push(Box::new(name.clone()));
        }

        if let Some(email) = &self.email_filter {
            params.push(Box::new(email.clone()));
        }

        params
    }
}

// 產品查詢構建器
#[derive(Debug)]
struct ProductQuery {
    min_price: Option<f64>,
    max_price: Option<f64>,
    tenant_id: String,
}

impl QueryBuilder for ProductQuery {
    fn build_query(&self) -> String {
        let mut conditions = vec!["tenant_id = $1".to_string()];
        let mut param_count = 1;

        if let Some(_) = self.min_price {
            param_count += 1;
            conditions.push(format!("price >= ${}", param_count));
        }

        if let Some(_) = self.max_price {
            param_count += 1;
            conditions.push(format!("price <= ${}", param_count));
        }

        format!("SELECT * FROM products WHERE {}", conditions.join(" AND "))
    }

    fn build_params(&self) -> Vec<Box<dyn PostgresParam>> {
        let mut params: Vec<Box<dyn PostgresParam>> = vec![Box::new(self.tenant_id.clone())];

        // 直接使用 f64 值，不要轉換為字符串
        if let Some(min_price) = self.min_price {
            params.push(Box::new(min_price));
        }

        if let Some(max_price) = self.max_price {
            params.push(Box::new(max_price));
        }

        params
    }
}
// 使用示例
async fn example_usage(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 初始化認證上下文
    let auth = AuthContext {
        user_id: "user123".to_string(),
        email: "user@example.com".to_string(),
        tenant_id: "tenant123".to_string(),
    };

    // 1. 基本的插入操作
    let insert_user = "INSERT INTO users (name, email, tenant_id) VALUES ($1, $2, $3)";
    pool.execute(
        insert_user,
        vec!["John Doe", "john@example.com", &auth.tenant_id],
    )
    .await?;

    // 2. 使用查詢構建器查詢用戶
    let user_query = UserQuery {
        name_filter: Some("John Doe".to_string()),
        email_filter: None,
        tenant_id: auth.tenant_id.clone(),
    };

    let query = user_query.build_query();
    let params = user_query.build_params();
    let users: Vec<User> = pool.fetch(&query, params).await?;
    info!("找到的用戶: {:?}", users);

    // 3. 產品查詢示例
    let product_query = ProductQuery {
        min_price: Some(100.0),
        max_price: Some(1000.0),
        tenant_id: auth.tenant_id.clone(),
    };

    let products: Vec<Product> = pool
        .fetch(&product_query.build_query(), product_query.build_params())
        .await?;
    info!("找到的產品: {:?}", products);

    // 4. 批量操作示例
    // 4. 批量操作示例
    let batch_insert = r#"
        INSERT INTO products (name, price, tenant_id)
        VALUES ($1, $2::double precision, $3), ($4, $5::double precision, $6)
    "#;

    pool.execute(
        batch_insert,
        vec![
            "Product 1",
            "100.0", // 保持為字串，但在 SQL 中做類型轉換
            &auth.tenant_id,
            "Product 2",
            "200.0", // 保持為字串，但在 SQL 中做類型轉換
            &auth.tenant_id,
        ],
    )
    .await?;

    Ok(())
}

// 建立資料庫 Schema 的函數
async fn setup_database(pool: &PgPool) -> Result<(), sqlx::Error> {
    // 創建用戶表
    let create_users = "
        CREATE TABLE IF NOT EXISTS users (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            email TEXT NOT NULL,
            tenant_id TEXT NOT NULL
        )";
    pool.execute(create_users, Vec::<String>::new()).await?;

    // 創建產品表
    let create_products = "
        CREATE TABLE IF NOT EXISTS products (
            id SERIAL PRIMARY KEY,
            name TEXT NOT NULL,
            price DOUBLE PRECISION NOT NULL,
            tenant_id TEXT NOT NULL
        )";
    pool.execute(create_products, Vec::<String>::new()).await?;

    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), sqlx::Error> {
    // 初始化日誌
    tracing_subscriber::fmt::init();

    // 設置連接池
    let pool = PgPool::connect("postgres://Rex@localhost:5432/mydb")
        .await
        .expect("無法連接到數據庫");

    // 設置數據庫 schema
    setup_database(&pool).await?;

    // 運行示例
    example_usage(&pool).await?;

    Ok(())
}
