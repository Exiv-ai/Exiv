-- Default Plugins Seeding
INSERT OR IGNORE INTO plugin_settings (plugin_id, is_active, allowed_permissions) VALUES 
('core.ks22', 1, '[]'),
('mind.deepseek', 1, '["NetworkAccess"]'),
('mind.cerebras', 1, '["NetworkAccess"]'),
('core.moderator', 1, '[]'),
('hal.cursor', 1, '[]'),
('bridge.python', 1, '[]');

-- Default Agents Seeding (Optional, can be customized)
INSERT OR IGNORE INTO agents (id, name, description, default_engine_id, status, metadata) VALUES 
('agent.vers_default', 'VERS Assistant', 'The primary system assistant.', 'mind.deepseek', 'online', '{}');
