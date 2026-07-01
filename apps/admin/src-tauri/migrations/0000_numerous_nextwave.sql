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
CREATE INDEX `idx_event_log_org` ON `event_log` (`org_id`,`hlc_ts`);