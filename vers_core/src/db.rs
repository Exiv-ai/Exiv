use sqlx::SqlitePool;
use tracing::info;

pub async fn init_db(pool: &SqlitePool, database_url: &str) -> anyhow::Result<()> {
    info!("Running database migrations...");

    // Run migrations from migrations/ directory
    sqlx::migrate!("./migrations").run(pool).await?;

    info!("Seeding dynamic configurations... URL: {}", database_url);
    // Seeds that might depend on runtime config (e.g. database_url)
    sqlx::query("INSERT OR REPLACE INTO plugin_configs (plugin_id, config_key, config_value) VALUES ('core.ks2_2', 'database_url', ?)")
        .bind(database_url)
        .execute(pool).await?;

    Ok(())
}
