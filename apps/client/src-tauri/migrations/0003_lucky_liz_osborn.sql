CREATE TABLE `ia_queries` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`text` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`view_id` text,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_ia_queries_org` ON `ia_queries` (`org_id`);--> statement-breakpoint
CREATE INDEX `idx_ia_queries_status` ON `ia_queries` (`org_id`,`status`);--> statement-breakpoint
CREATE TABLE `view_definitions` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`sql` text NOT NULL,
	`entity` text NOT NULL,
	`components_json` text NOT NULL,
	`root` text NOT NULL,
	`meta_json` text NOT NULL,
	`created_by` text NOT NULL,
	`tags` text,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_view_definitions_org` ON `view_definitions` (`org_id`);