CREATE TABLE IF NOT EXISTS t_c (id INTEGER PRIMARY KEY, data TEXT NOT NULL);
--> statement-breakpoint
INSERT INTO t_c (id, data) VALUES (1, 'hello');
--> statement-breakpoint
NOT_VALID_SQL
