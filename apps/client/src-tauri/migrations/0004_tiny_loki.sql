PRAGMA foreign_keys=OFF;--> statement-breakpoint
CREATE TABLE `__new_invoice_items` (
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
INSERT INTO `__new_invoice_items`("org_id", "invoice_id", "line_id", "product_id", "qty", "price", "change_time", "node_id") SELECT "org_id", "invoice_id", "line_id", "product_id", "qty", "price", "change_time", "node_id" FROM `invoice_items`;--> statement-breakpoint
DROP TABLE `invoice_items`;--> statement-breakpoint
ALTER TABLE `__new_invoice_items` RENAME TO `invoice_items`;--> statement-breakpoint
PRAGMA foreign_keys=ON;--> statement-breakpoint
CREATE INDEX `idx_invoice_items_invoice` ON `invoice_items` (`org_id`,`invoice_id`);--> statement-breakpoint
CREATE TABLE `__new_order_items` (
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
INSERT INTO `__new_order_items`("org_id", "order_id", "line_id", "product_id", "qty", "change_time", "node_id") SELECT "org_id", "order_id", "line_id", "product_id", "qty", "change_time", "node_id" FROM `order_items`;--> statement-breakpoint
DROP TABLE `order_items`;--> statement-breakpoint
ALTER TABLE `__new_order_items` RENAME TO `order_items`;--> statement-breakpoint
CREATE INDEX `idx_order_items_order` ON `order_items` (`org_id`,`order_id`);