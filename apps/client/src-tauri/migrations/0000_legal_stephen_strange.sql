CREATE TABLE `customers` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `event_log` (
	`id` integer PRIMARY KEY AUTOINCREMENT NOT NULL,
	`org_id` text NOT NULL,
	`key` text NOT NULL,
	`event_type` text NOT NULL,
	`hlc_ts` integer NOT NULL,
	`hlc_count` integer NOT NULL,
	`hlc_node` text NOT NULL,
	`schema_version` integer DEFAULT 1 NOT NULL,
	`entity` text DEFAULT '' NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`created_at` integer DEFAULT (unixepoch('now') * 1000) NOT NULL
);
--> statement-breakpoint
CREATE UNIQUE INDEX `event_log_key_unique` ON `event_log` (`key`);--> statement-breakpoint
CREATE INDEX `idx_event_log_org` ON `event_log` (`org_id`,`hlc_ts`);--> statement-breakpoint
CREATE TABLE `heartbeats` (
	`org_id` text NOT NULL,
	`node_id` text NOT NULL,
	`ts` integer DEFAULT 0 NOT NULL,
	`data` text DEFAULT '{}' NOT NULL,
	PRIMARY KEY(`org_id`, `node_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_heartbeats_org` ON `heartbeats` (`org_id`,`node_id`);--> statement-breakpoint
CREATE TABLE `hlc_tracker` (
	`org_id` text NOT NULL,
	`entity` text NOT NULL,
	`doc_id` text NOT NULL,
	`hlc_ts` integer DEFAULT 0 NOT NULL,
	`hlc_count` integer DEFAULT 0 NOT NULL,
	`hlc_node` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `entity`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `invoices` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `members` (
	`org_id` text NOT NULL,
	`node_id` text NOT NULL,
	`data` text DEFAULT '{}' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	PRIMARY KEY(`org_id`, `node_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_members_org` ON `members` (`org_id`);--> statement-breakpoint
CREATE TABLE `orders` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `payroll` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `products` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE TABLE `roles` (
	`org_id` text NOT NULL,
	`role_name` text NOT NULL,
	`data` text DEFAULT '{}' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	PRIMARY KEY(`org_id`, `role_name`)
);
--> statement-breakpoint
CREATE INDEX `idx_roles_org` ON `roles` (`org_id`);--> statement-breakpoint
CREATE TABLE `suppliers` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`payload` text DEFAULT '{}' NOT NULL,
	`fts_title` text DEFAULT '' NOT NULL,
	`fts_body` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
