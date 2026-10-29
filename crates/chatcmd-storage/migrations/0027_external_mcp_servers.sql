CREATE TABLE IF NOT EXISTS external_mcp_servers (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    description TEXT,
    enabled INTEGER NOT NULL DEFAULT 1,
    transport_type TEXT NOT NULL,
    config_json TEXT NOT NULL,
    created_at_ms INTEGER NOT NULL,
    updated_at_ms INTEGER NOT NULL
);
