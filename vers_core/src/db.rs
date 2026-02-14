use sqlx::SqlitePool;
use tracing::info;
use async_trait::async_trait;
use vers_shared::PluginDataStore;

pub struct SqliteDataStore {
    pool: SqlitePool,
}

impl SqliteDataStore {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl PluginDataStore for SqliteDataStore {
    async fn set_json(&self, plugin_id: &str, key: &str, value: serde_json::Value) -> anyhow::Result<()> {
        let val_str = serde_json::to_string(&value)?;
        sqlx::query("INSERT OR REPLACE INTO plugin_data (plugin_id, key, value) VALUES (?, ?, ?)")
            .bind(plugin_id)
            .bind(key)
            .bind(val_str)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn get_json(&self, plugin_id: &str, key: &str) -> anyhow::Result<Option<serde_json::Value>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT value FROM plugin_data WHERE plugin_id = ? AND key = ?")
            .bind(plugin_id)
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
            
        if let Some((val_str,)) = row {
            let val = serde_json::from_str(&val_str)?;
            Ok(Some(val))
        } else {
            Ok(None)
        }
    }
}

pub async fn init_db(pool: &SqlitePool, database_url: &str) -> anyhow::Result<()> {
    info!("Running database migrations...");

    // Run migrations from migrations/ directory
    sqlx::migrate!("./migrations").run(pool).await?;

    info!("Seeding dynamic configurations... URL: {}", database_url);
    
    // Seed default plugins
    let defaults = vec![
        ("core.ks2_2", "[]"),
        ("mind.deepseek", "[\"NetworkAccess\"]"),
        ("mind.cerebras", "[\"NetworkAccess\"]"),
        ("hal.cursor", "[]"),
        ("bridge.python", "[]"),
    ];
    for (id, perms) in defaults {
        sqlx::query(
            "INSERT OR IGNORE INTO plugin_settings (plugin_id, is_active, allowed_permissions) VALUES (?, 1, ?)",
        )
        .bind(id)
        .bind(perms)
        .execute(pool)
        .await?;
    }

    // Seeds that might depend on runtime config (e.g. database_url)
    sqlx::query("INSERT OR REPLACE INTO plugin_configs (plugin_id, config_key, config_value) VALUES ('core.ks2_2', 'database_url', ?)")
        .bind(database_url)
        .execute(pool).await?;

    Ok(())
}
