CREATE TABLE `admin_devices` (
	`org_id` text NOT NULL,
	`node_id` text NOT NULL,
	`active` integer DEFAULT true NOT NULL,
	`role` text DEFAULT 'sales' NOT NULL,
	`person` text DEFAULT '' NOT NULL,
	`name` text DEFAULT '' NOT NULL,
	`device_addr` text DEFAULT '' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	PRIMARY KEY(`org_id`, `node_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_admin_devices_org` ON `admin_devices` (`org_id`);--> statement-breakpoint
CREATE TABLE `admin_orgs` (
	`name` text PRIMARY KEY NOT NULL,
	`topic_id` text NOT NULL,
	`created_at` integer DEFAULT (unixepoch('now') * 1000) NOT NULL
);
--> statement-breakpoint
CREATE TABLE `admin_roles` (
	`org_id` text NOT NULL,
	`role_name` text NOT NULL,
	`can_open` text DEFAULT '[]' NOT NULL,
	`can_write` text DEFAULT '[]' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	PRIMARY KEY(`org_id`, `role_name`)
);
--> statement-breakpoint
CREATE INDEX `idx_admin_roles_org` ON `admin_roles` (`org_id`);
