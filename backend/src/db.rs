use sqlx::{postgres::PgPoolOptions, PgPool};
use tracing::info;

pub struct Database {
    pool: PgPool,
}

impl Database {
    pub async fn new(database_url: &str) -> anyhow::Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(50)
            .connect(database_url)
            .await?;

        // Run migrations on startup
        info!("Running database migrations...");
        match sqlx::migrate!("./migrations").run(&pool).await {
            Ok(_) => info!("Database migrations completed successfully"),
            Err(e) => {
                if e.to_string().contains("previously applied but has been modified") {
                    info!("Migration checksum mismatch detected, ignoring (migrations already applied)");
                } else {
                    return Err(e.into());
                }
            }
        }

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
