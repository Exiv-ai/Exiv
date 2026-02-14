-- Initial schema for VERS Kernel (Consolidated)

CREATE TABLE IF NOT EXISTS plugin_settings (
    plugin_id TEXT PRIMARY KEY,
    is_active BOOLEAN NOT NULL DEFAULT 1
);

CREATE TABLE IF NOT EXISTS plugin_configs (
    plugin_id TEXT,
    config_key TEXT,
    config_value TEXT,
    PRIMARY KEY(plugin_id, config_key)
);

CREATE TABLE IF NOT EXISTS agents (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    default_engine_id TEXT NOT NULL,
    status TEXT DEFAULT 'offline',
    metadata TEXT DEFAULT '{}'
);

-- Initial Seeds
INSERT OR IGNORE INTO agents (id, name, description, default_engine_id, status, metadata) 
VALUES ('agent.karin', 'Karin', 'Vers-native Karin Agent / General Intelligence', 'mind.deepseek', 'online', '{}');

INSERT OR IGNORE INTO agents (id, name, description, default_engine_id, status, metadata) 
VALUES ('agent.analyst', 'Python Analyst', 'Advanced Data Analyst powered by Python Bridge', 'bridge.python', 'online', '{"preferred_memory": "core.ks2_2"}');