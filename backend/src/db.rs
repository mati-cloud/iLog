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
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
        info!("Database migrations completed successfully");

        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
