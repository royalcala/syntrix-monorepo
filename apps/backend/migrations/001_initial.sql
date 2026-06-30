CREATE EXTENSION IF NOT EXISTS "pgcrypto";

CREATE TABLE wallets (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    balance_cents BIGINT NOT NULL DEFAULT 0,
    reserved_cents BIGINT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE tenants (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    org_id VARCHAR NOT NULL UNIQUE,
    name VARCHAR,
    public_key_hex VARCHAR NOT NULL,
    api_key_hash VARCHAR NOT NULL,
    status VARCHAR DEFAULT 'active',
    wallet_id UUID REFERENCES wallets(id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE wallet_transactions (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    wallet_id UUID REFERENCES wallets(id) NOT NULL,
    type VARCHAR NOT NULL,
    amount_cents BIGINT NOT NULL,
    service VARCHAR,
    stripe_payment_intent_id VARCHAR,
    idempotency_key VARCHAR UNIQUE,
    metadata JSONB,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE service_rate_cards (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    service VARCHAR NOT NULL,
    unit_label VARCHAR NOT NULL,
    rate_cents_per_unit BIGINT NOT NULL,
    UNIQUE(service)
);

CREATE TABLE service_usage_log (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) NOT NULL,
    service VARCHAR NOT NULL,
    quantity BIGINT NOT NULL,
    cost_cents BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE gateway_api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) NOT NULL,
    key_hash VARCHAR NOT NULL UNIQUE,
    name VARCHAR,
    rate_limit INTEGER DEFAULT 100,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    status VARCHAR DEFAULT 'active'
);

CREATE TABLE key_recovery_blobs (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) NOT NULL,
    device_label VARCHAR NOT NULL,
    encrypted_blob BYTEA NOT NULL,
    encryption_salt VARCHAR NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE cfdi_stamps (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) NOT NULL,
    pac_reference VARCHAR NOT NULL,
    cfdi_uuid VARCHAR,
    xml_in TEXT,
    xml_out TEXT,
    status VARCHAR DEFAULT 'pending',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE relay_nodes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    tenant_id UUID REFERENCES tenants(id) NOT NULL,
    node_id_hex VARCHAR NOT NULL,
    endpoint VARCHAR,
    status VARCHAR DEFAULT 'offline',
    heartbeat_at TIMESTAMPTZ
);

CREATE TABLE operators (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    email VARCHAR NOT NULL UNIQUE,
    password_hash VARCHAR NOT NULL,
    totp_secret VARCHAR,
    totp_verified BOOLEAN DEFAULT FALSE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE contact_inquiries (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR,
    email VARCHAR NOT NULL,
    message TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Seed rate cards
INSERT INTO service_rate_cards (service, unit_label, rate_cents_per_unit) VALUES
    ('cfdi_stamp', 'timbre', 350),
    ('ai_gateway', '1M tokens', 75),
    ('email', 'email', 5),
    ('sms', 'message', 15),
    ('whatsapp', 'message', 25),
    ('relay_node', 'month', 5000),
    ('key_recovery', 'GB/month', 1000);
