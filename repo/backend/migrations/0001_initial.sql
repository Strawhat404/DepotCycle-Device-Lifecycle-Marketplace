PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS roles (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE
);

INSERT OR IGNORE INTO roles (name) VALUES
('Shopper'),
('Inventory Clerk'),
('Manager'),
('Support Agent'),
('Administrator');

CREATE TABLE IF NOT EXISTS users (
    id TEXT PRIMARY KEY NOT NULL,
    username TEXT NOT NULL UNIQUE,
    password_hash TEXT NOT NULL,
    role_id INTEGER NOT NULL,
    display_name_enc TEXT,
    phone_enc TEXT,
    failed_login_attempts INTEGER NOT NULL DEFAULT 0,
    locked_until TEXT,
    last_login_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(role_id) REFERENCES roles(id)
);

CREATE TABLE IF NOT EXISTS sessions (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    token_hash TEXT NOT NULL UNIQUE,
    ip_address TEXT,
    user_agent TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    last_activity_at TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS campuses (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL,
    zip_code TEXT NOT NULL,
    latitude REAL NOT NULL,
    longitude REAL NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS listing_conditions (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    code TEXT NOT NULL UNIQUE,
    label TEXT NOT NULL
);

INSERT OR IGNORE INTO listing_conditions (code, label) VALUES
('new', 'New'),
('open_box', 'Open Box'),
('refurbished', 'Refurbished'),
('good', 'Good'),
('fair', 'Fair'),
('for_parts', 'For Parts');

CREATE TABLE IF NOT EXISTS taxonomy_nodes (
    id TEXT PRIMARY KEY NOT NULL,
    parent_id TEXT,
    name TEXT NOT NULL,
    slug TEXT NOT NULL UNIQUE,
    level INTEGER NOT NULL,
    seo_title TEXT,
    seo_description TEXT,
    seo_keywords TEXT,
    topic_page_path TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    FOREIGN KEY(parent_id) REFERENCES taxonomy_nodes(id)
);

CREATE TABLE IF NOT EXISTS taxonomy_tags (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    slug TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS taxonomy_keywords (
    id TEXT PRIMARY KEY NOT NULL,
    keyword TEXT NOT NULL UNIQUE
);

CREATE TABLE IF NOT EXISTS taxonomy_node_tags (
    node_id TEXT NOT NULL,
    tag_id TEXT NOT NULL,
    PRIMARY KEY(node_id, tag_id),
    FOREIGN KEY(node_id) REFERENCES taxonomy_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(tag_id) REFERENCES taxonomy_tags(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS taxonomy_node_keywords (
    node_id TEXT NOT NULL,
    keyword_id TEXT NOT NULL,
    PRIMARY KEY(node_id, keyword_id),
    FOREIGN KEY(node_id) REFERENCES taxonomy_nodes(id) ON DELETE CASCADE,
    FOREIGN KEY(keyword_id) REFERENCES taxonomy_keywords(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS listings (
    id TEXT PRIMARY KEY NOT NULL,
    seller_user_id TEXT,
    campus_id TEXT,
    taxonomy_node_id TEXT,
    condition_id INTEGER,
    title TEXT NOT NULL,
    description TEXT,
    price_cents INTEGER NOT NULL DEFAULT 0,
    currency TEXT NOT NULL DEFAULT 'USD',
    status TEXT NOT NULL DEFAULT 'draft',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(seller_user_id) REFERENCES users(id),
    FOREIGN KEY(campus_id) REFERENCES campuses(id),
    FOREIGN KEY(taxonomy_node_id) REFERENCES taxonomy_nodes(id),
    FOREIGN KEY(condition_id) REFERENCES listing_conditions(id)
);

CREATE TABLE IF NOT EXISTS listing_media (
    id TEXT PRIMARY KEY NOT NULL,
    listing_id TEXT,
    storage_path TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    media_kind TEXT NOT NULL,
    chunk_group TEXT,
    playback_token TEXT NOT NULL UNIQUE,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(listing_id) REFERENCES listings(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_chunks (
    id TEXT PRIMARY KEY NOT NULL,
    media_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    chunk_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(media_id) REFERENCES listing_media(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS inventory_devices (
    id TEXT PRIMARY KEY NOT NULL,
    listing_id TEXT,
    serial_number TEXT,
    asset_tag TEXT,
    status TEXT NOT NULL,
    campus_id TEXT,
    assigned_to_user_id TEXT,
    metadata_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(listing_id) REFERENCES listings(id),
    FOREIGN KEY(campus_id) REFERENCES campuses(id),
    FOREIGN KEY(assigned_to_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS inventory_documents (
    id TEXT PRIMARY KEY NOT NULL,
    doc_type TEXT NOT NULL CHECK (doc_type IN ('receiving', 'issuing', 'transfer', 'return', 'loan', 'scrap')),
    reference_no TEXT NOT NULL UNIQUE,
    source_campus_id TEXT,
    target_campus_id TEXT,
    device_id TEXT,
    related_user_id TEXT,
    notes TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(source_campus_id) REFERENCES campuses(id),
    FOREIGN KEY(target_campus_id) REFERENCES campuses(id),
    FOREIGN KEY(device_id) REFERENCES inventory_devices(id),
    FOREIGN KEY(related_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS ledger_change_records (
    id TEXT PRIMARY KEY NOT NULL,
    table_name TEXT NOT NULL,
    record_id TEXT NOT NULL,
    before_json TEXT,
    after_json TEXT,
    operator_user_id TEXT,
    occurred_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    related_document_id TEXT,
    FOREIGN KEY(operator_user_id) REFERENCES users(id),
    FOREIGN KEY(related_document_id) REFERENCES inventory_documents(id)
);

CREATE TABLE IF NOT EXISTS shipment_orders (
    id TEXT PRIMARY KEY NOT NULL,
    order_number TEXT NOT NULL UNIQUE,
    listing_id TEXT,
    device_id TEXT,
    from_campus_id TEXT,
    to_campus_id TEXT,
    status TEXT NOT NULL,
    sla_due_at TEXT,
    shipped_at TEXT,
    delivered_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(listing_id) REFERENCES listings(id),
    FOREIGN KEY(device_id) REFERENCES inventory_devices(id),
    FOREIGN KEY(from_campus_id) REFERENCES campuses(id),
    FOREIGN KEY(to_campus_id) REFERENCES campuses(id)
);

CREATE TABLE IF NOT EXISTS shipment_status_history (
    id TEXT PRIMARY KEY NOT NULL,
    shipment_order_id TEXT NOT NULL,
    from_status TEXT,
    to_status TEXT NOT NULL,
    changed_by TEXT,
    changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(shipment_order_id) REFERENCES shipment_orders(id) ON DELETE CASCADE,
    FOREIGN KEY(changed_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS service_level_agreements (
    id TEXT PRIMARY KEY NOT NULL,
    entity_type TEXT NOT NULL,
    entity_id TEXT NOT NULL,
    due_at TEXT NOT NULL,
    met_at TEXT,
    status TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS ratings (
    id TEXT PRIMARY KEY NOT NULL,
    listing_id TEXT,
    user_id TEXT,
    score INTEGER NOT NULL CHECK (score BETWEEN 1 AND 5),
    comments TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(listing_id) REFERENCES listings(id),
    FOREIGN KEY(user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS appeal_tickets (
    id TEXT PRIMARY KEY NOT NULL,
    ticket_no TEXT NOT NULL UNIQUE,
    listing_id TEXT,
    shipment_order_id TEXT,
    opened_by_user_id TEXT,
    assigned_to_user_id TEXT,
    status TEXT NOT NULL,
    reason TEXT NOT NULL,
    resolution TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(listing_id) REFERENCES listings(id),
    FOREIGN KEY(shipment_order_id) REFERENCES shipment_orders(id),
    FOREIGN KEY(opened_by_user_id) REFERENCES users(id),
    FOREIGN KEY(assigned_to_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS cohorts (
    id TEXT PRIMARY KEY NOT NULL,
    name TEXT NOT NULL UNIQUE,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS cohort_assignments (
    id TEXT PRIMARY KEY NOT NULL,
    cohort_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    assigned_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(cohort_id) REFERENCES cohorts(id) ON DELETE CASCADE,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS event_logs (
    id TEXT PRIMARY KEY NOT NULL,
    event_name TEXT NOT NULL,
    user_id TEXT,
    cohort_id TEXT,
    session_id TEXT,
    properties_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(user_id) REFERENCES users(id),
    FOREIGN KEY(cohort_id) REFERENCES cohorts(id),
    FOREIGN KEY(session_id) REFERENCES sessions(id)
);

CREATE TABLE IF NOT EXISTS feature_flags (
    id TEXT PRIMARY KEY NOT NULL,
    key TEXT NOT NULL UNIQUE,
    description TEXT,
    enabled INTEGER NOT NULL DEFAULT 0,
    rollout_percent INTEGER NOT NULL DEFAULT 0,
    audience_rules_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE TABLE IF NOT EXISTS local_credentials (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    username TEXT NOT NULL,
    secret_enc TEXT NOT NULL,
    notes TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS companion_credentials (
    id TEXT PRIMARY KEY NOT NULL,
    label TEXT NOT NULL,
    provider TEXT NOT NULL,
    endpoint TEXT,
    username TEXT NOT NULL,
    secret_enc TEXT NOT NULL,
    notes TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS templates (
    id TEXT PRIMARY KEY NOT NULL,
    kind TEXT NOT NULL,
    key TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    version INTEGER NOT NULL DEFAULT 1,
    is_active INTEGER NOT NULL DEFAULT 1,
    updated_by TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(updated_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS announcements (
    id TEXT PRIMARY KEY NOT NULL,
    title TEXT NOT NULL,
    body TEXT NOT NULL,
    severity TEXT NOT NULL,
    starts_at TEXT,
    ends_at TEXT,
    created_by TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(created_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS announcement_deliveries (
    id TEXT PRIMARY KEY NOT NULL,
    announcement_id TEXT NOT NULL,
    user_id TEXT NOT NULL,
    delivered_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    read_at TEXT,
    FOREIGN KEY(announcement_id) REFERENCES announcements(id) ON DELETE CASCADE,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS admin_audit_trails (
    id TEXT PRIMARY KEY NOT NULL,
    actor_user_id TEXT NOT NULL,
    action TEXT NOT NULL,
    target_table TEXT NOT NULL,
    target_id TEXT NOT NULL,
    details_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(actor_user_id) REFERENCES users(id)
);

CREATE TRIGGER IF NOT EXISTS trg_admin_audit_no_update
BEFORE UPDATE ON admin_audit_trails
BEGIN
    SELECT RAISE(ABORT, 'admin_audit_trails is append-only');
END;

CREATE TRIGGER IF NOT EXISTS trg_admin_audit_no_delete
BEFORE DELETE ON admin_audit_trails
BEGIN
    SELECT RAISE(ABORT, 'admin_audit_trails is append-only');
END;

CREATE TRIGGER IF NOT EXISTS trg_ledger_no_update
BEFORE UPDATE ON ledger_change_records
BEGIN
    SELECT RAISE(ABORT, 'ledger_change_records is append-only');
END;

CREATE TRIGGER IF NOT EXISTS trg_ledger_no_delete
BEFORE DELETE ON ledger_change_records
BEGIN
    SELECT RAISE(ABORT, 'ledger_change_records is append-only');
END;
