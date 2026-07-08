CREATE TABLE `cdc_cursor` (
	`org_id` text PRIMARY KEY NOT NULL,
	`last_change_id` integer DEFAULT 0 NOT NULL,
	`updated_at` integer DEFAULT (unixepoch('now') * 1000) NOT NULL
);
--> statement-breakpoint
CREATE TABLE `customers` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`name` text NOT NULL,
	`tax_id` text,
	`address` text,
	`phone` text,
	`email` text,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_customers_org` ON `customers` (`org_id`);--> statement-breakpoint
CREATE TABLE `heartbeats` (
	`org_id` text NOT NULL,
	`node_id` text NOT NULL,
	`ts` integer DEFAULT 0 NOT NULL,
	`data` text DEFAULT '{}' NOT NULL,
	PRIMARY KEY(`org_id`, `node_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_heartbeats_org` ON `heartbeats` (`org_id`,`node_id`);--> statement-breakpoint
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
CREATE TABLE `invoice_items` (
	`org_id` text NOT NULL,
	`invoice_id` text NOT NULL,
	`line_id` text NOT NULL,
	`product_id` text NOT NULL,
	`qty` real NOT NULL,
	`price` real NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `invoice_id`, `line_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_invoice_items_invoice` ON `invoice_items` (`org_id`,`invoice_id`);--> statement-breakpoint
CREATE TABLE `invoices` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`customer_id` text NOT NULL,
	`amount` real NOT NULL,
	`status` text DEFAULT 'draft' NOT NULL,
	`tax_rate` real DEFAULT 0.16 NOT NULL,
	`date` text NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_invoices_org` ON `invoices` (`org_id`);--> statement-breakpoint
CREATE INDEX `idx_invoices_customer` ON `invoices` (`org_id`,`customer_id`);--> statement-breakpoint
CREATE TABLE `members` (
	`org_id` text NOT NULL,
	`node_id` text NOT NULL,
	`data` text DEFAULT '{}' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	PRIMARY KEY(`org_id`, `node_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_members_org` ON `members` (`org_id`);--> statement-breakpoint
CREATE TABLE `order_items` (
	`org_id` text NOT NULL,
	`order_id` text NOT NULL,
	`line_id` text NOT NULL,
	`product_id` text NOT NULL,
	`qty` real NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `order_id`, `line_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_order_items_order` ON `order_items` (`org_id`,`order_id`);--> statement-breakpoint
CREATE TABLE `orders` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`customer_id` text NOT NULL,
	`status` text DEFAULT 'pending' NOT NULL,
	`date` text NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_orders_org` ON `orders` (`org_id`);--> statement-breakpoint
CREATE INDEX `idx_orders_customer` ON `orders` (`org_id`,`customer_id`);--> statement-breakpoint
CREATE TABLE `payroll` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`employee_id` text NOT NULL,
	`employee_name` text NOT NULL,
	`period` text NOT NULL,
	`gross_amount` real NOT NULL,
	`deductions` real DEFAULT 0 NOT NULL,
	`net_amount` real NOT NULL,
	`status` text DEFAULT 'draft' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_payroll_org` ON `payroll` (`org_id`);--> statement-breakpoint
CREATE TABLE `products` (
	`org_id` text NOT NULL,
	`doc_id` text NOT NULL,
	`name` text NOT NULL,
	`sku` text,
	`price` real NOT NULL,
	`unit` text DEFAULT 'pza' NOT NULL,
	`category` text DEFAULT 'general' NOT NULL,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_products_org` ON `products` (`org_id`);--> statement-breakpoint
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
	`name` text NOT NULL,
	`tax_id` text,
	`address` text,
	`phone` text,
	`email` text,
	`change_time` integer DEFAULT (unixepoch('now') * 1000) NOT NULL,
	`node_id` text DEFAULT '' NOT NULL,
	PRIMARY KEY(`org_id`, `doc_id`)
);
--> statement-breakpoint
CREATE INDEX `idx_suppliers_org` ON `suppliers` (`org_id`);--> statement-breakpoint
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