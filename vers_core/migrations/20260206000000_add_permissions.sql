-- Add allowed_permissions column to plugin_settings
-- Using TEXT to store a JSON array of permissions
ALTER TABLE plugin_settings ADD COLUMN allowed_permissions TEXT DEFAULT '[]';

-- Update existing plugins with sensible default permissions
-- mind.deepseek needs NetworkAccess
UPDATE plugin_settings SET allowed_permissions = '["NetworkAccess"]' WHERE plugin_id = 'mind.deepseek';
UPDATE plugin_settings SET allowed_permissions = '["NetworkAccess"]' WHERE plugin_id = 'mind.cerebras';
-- core.ks2_2 might need MemoryRead/Write if it were a plugin, but it's built-in capability. 
-- For now, let's keep it safe.
