CREATE TABLE IF NOT EXISTS user_settings (
    user_id TEXT PRIMARY KEY NOT NULL,
    recommendations_enabled INTEGER NOT NULL DEFAULT 1,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS favorites (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    listing_id TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(user_id, listing_id),
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE,
    FOREIGN KEY(listing_id) REFERENCES listings(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS orders (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'placed',
    total_cents INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS order_items (
    id TEXT PRIMARY KEY NOT NULL,
    order_id TEXT NOT NULL,
    listing_id TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_price_cents INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(order_id) REFERENCES orders(id) ON DELETE CASCADE,
    FOREIGN KEY(listing_id) REFERENCES listings(id)
);

CREATE TABLE IF NOT EXISTS search_history (
    id TEXT PRIMARY KEY NOT NULL,
    user_id TEXT NOT NULL,
    query_text TEXT NOT NULL,
    filters_json TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_upload_sessions (
    id TEXT PRIMARY KEY NOT NULL,
    created_by TEXT NOT NULL,
    file_name TEXT NOT NULL,
    mime_type TEXT NOT NULL,
    total_chunks INTEGER NOT NULL,
    uploaded_chunks INTEGER NOT NULL DEFAULT 0,
    status TEXT NOT NULL DEFAULT 'created',
    target_listing_id TEXT,
    expected_sha256 TEXT,
    assembled_path TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(created_by) REFERENCES users(id),
    FOREIGN KEY(target_listing_id) REFERENCES listings(id)
);

CREATE TABLE IF NOT EXISTS media_upload_chunks (
    id TEXT PRIMARY KEY NOT NULL,
    session_id TEXT NOT NULL,
    chunk_index INTEGER NOT NULL,
    chunk_path TEXT NOT NULL,
    sha256 TEXT NOT NULL,
    size_bytes INTEGER NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE(session_id, chunk_index),
    FOREIGN KEY(session_id) REFERENCES media_upload_sessions(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS media_playback_tokens (
    id TEXT PRIMARY KEY NOT NULL,
    media_id TEXT NOT NULL,
    token TEXT NOT NULL UNIQUE,
    issued_to_user_id TEXT NOT NULL,
    expires_at TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(media_id) REFERENCES listing_media(id) ON DELETE CASCADE,
    FOREIGN KEY(issued_to_user_id) REFERENCES users(id) ON DELETE CASCADE
);

CREATE TABLE IF NOT EXISTS inventory_document_lines (
    id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL,
    device_id TEXT NOT NULL,
    quantity INTEGER NOT NULL,
    unit_value_cents INTEGER NOT NULL,
    target_campus_id TEXT,
    notes TEXT,
    FOREIGN KEY(document_id) REFERENCES inventory_documents(id) ON DELETE CASCADE,
    FOREIGN KEY(device_id) REFERENCES inventory_devices(id),
    FOREIGN KEY(target_campus_id) REFERENCES campuses(id)
);

CREATE TABLE IF NOT EXISTS approval_requests (
    id TEXT PRIMARY KEY NOT NULL,
    document_id TEXT NOT NULL UNIQUE,
    status TEXT NOT NULL DEFAULT 'pending',
    reason TEXT NOT NULL,
    requested_by TEXT NOT NULL,
    approved_by TEXT,
    approved_at TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(document_id) REFERENCES inventory_documents(id) ON DELETE CASCADE,
    FOREIGN KEY(requested_by) REFERENCES users(id),
    FOREIGN KEY(approved_by) REFERENCES users(id)
);

ALTER TABLE inventory_documents ADD COLUMN workflow_status TEXT NOT NULL DEFAULT 'draft';
ALTER TABLE shipment_orders ADD COLUMN carrier_name TEXT;
ALTER TABLE shipment_orders ADD COLUMN tracking_number TEXT;
ALTER TABLE shipment_orders ADD COLUMN integration_enabled INTEGER NOT NULL DEFAULT 0;

CREATE TABLE IF NOT EXISTS after_sales_cases (
    id TEXT PRIMARY KEY NOT NULL,
    order_id TEXT,
    case_type TEXT NOT NULL CHECK (case_type IN ('return', 'exchange', 'refund')),
    status TEXT NOT NULL DEFAULT 'requested',
    opened_by_user_id TEXT NOT NULL,
    assigned_to_user_id TEXT,
    reason TEXT NOT NULL,
    first_response_due_at TEXT NOT NULL,
    final_decision_due_at TEXT NOT NULL,
    resolution_notes TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(order_id) REFERENCES orders(id),
    FOREIGN KEY(opened_by_user_id) REFERENCES users(id),
    FOREIGN KEY(assigned_to_user_id) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS after_sales_status_history (
    id TEXT PRIMARY KEY NOT NULL,
    case_id TEXT NOT NULL,
    from_status TEXT,
    to_status TEXT NOT NULL,
    changed_by TEXT NOT NULL,
    changed_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(case_id) REFERENCES after_sales_cases(id) ON DELETE CASCADE,
    FOREIGN KEY(changed_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS after_sales_evidence (
    id TEXT PRIMARY KEY NOT NULL,
    case_id TEXT NOT NULL,
    media_id TEXT NOT NULL,
    uploaded_by TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(case_id) REFERENCES after_sales_cases(id) ON DELETE CASCADE,
    FOREIGN KEY(media_id) REFERENCES listing_media(id),
    FOREIGN KEY(uploaded_by) REFERENCES users(id)
);

CREATE TABLE IF NOT EXISTS rating_reviews (
    id TEXT PRIMARY KEY NOT NULL,
    rating_id TEXT NOT NULL UNIQUE,
    review_status TEXT NOT NULL DEFAULT 'pending',
    reviewed_by TEXT,
    reviewed_at TEXT,
    notes TEXT,
    FOREIGN KEY(rating_id) REFERENCES ratings(id) ON DELETE CASCADE,
    FOREIGN KEY(reviewed_by) REFERENCES users(id)
);
