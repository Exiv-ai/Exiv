# Refactoring Plan: Improving Principle Adherence (Phase 1)

## 1. Goal
- Decouple plugin-specific storage (Principle 4).
- Simplify Kernel bootstrap (Principle 1).

## 2. Changes

### A. Plugin-Driven Storage (Data Sovereignty)
- **Target**: `plugin_ks2_2`
- **Action**: Implement `on_plugin_init` in `Ks2_2Plugin` to create the `memories` table if it doesn't exist.
- **Kernel Action**: Remove `memories` table from `vers_core/migrations/20260205000000_init.sql`.

### B. Decoupled Bootstrap (Core Minimalism)
- **Target**: `vers_core/src/managers.rs` and `main.rs`
- **Action**:
    - Add `register_builtin_plugins` helper to `PluginManager`.
    - Reduce boilerplate in `main.rs`.

## 3. Detailed Steps

### Step 1: Update Ks2_2Plugin
In `projects/vers_project/vers_plugins/plugin_ks2_2/src/lib.rs`:
```rust
#[async_trait]
impl Plugin for Ks2_2Plugin {
    async fn on_plugin_init(...) -> anyhow::Result<()> {
        sqlx::query(
            "CREATE TABLE IF NOT EXISTS memories (
                id TEXT PRIMARY KEY,
                content TEXT NOT NULL,
                metadata TEXT,
                timestamp DATETIME DEFAULT CURRENT_TIMESTAMP
            )"
        ).execute(&self.pool).await?;
        Ok(())
    }
}
```

### Step 2: Cleanup Kernel Migration
In `projects/vers_project/vers_core/migrations/20260205000000_init.sql`:
- Delete the `CREATE TABLE IF NOT EXISTS memories` block.

### Step 3: Refactor PluginManager registration
In `projects/vers_project/vers_core/src/managers.rs`:
- Add a method to register all known "built-in" plugins to keep `main.rs` focused on infrastructure.

## 4. Verification
- Run the system and check if `memories` table is created automatically.
- Ensure `agent.karin` can still store/recall memories.
