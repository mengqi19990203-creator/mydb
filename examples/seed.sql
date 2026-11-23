-- 创建一个用于试用 mydb 的 SQLite 示例库
DROP VIEW IF EXISTS active_users;
DROP TABLE IF EXISTS order_items;
DROP TABLE IF EXISTS orders;
DROP TABLE IF EXISTS users;

CREATE TABLE users (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  name TEXT NOT NULL,
  email TEXT NOT NULL UNIQUE,
  city TEXT,
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE orders (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  user_id INTEGER NOT NULL REFERENCES users(id),
  amount REAL NOT NULL,
  status TEXT NOT NULL DEFAULT 'pending',
  created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE TABLE order_items (
  id INTEGER PRIMARY KEY AUTOINCREMENT,
  order_id INTEGER NOT NULL REFERENCES orders(id),
  product TEXT NOT NULL,
  quantity INTEGER NOT NULL DEFAULT 1,
  unit_price REAL NOT NULL
);

INSERT INTO users (name, email, city) VALUES
  ('张伟', 'zhangwei@example.com', '上海'),
  ('李娜', 'lina@example.com', '北京'),
  ('王强', 'wangqiang@example.com', '深圳'),
  ('陈静', 'chenjing@example.com', '杭州');

INSERT INTO orders (user_id, amount, status) VALUES
  (1, 299.00, 'paid'),
  (1, 89.50, 'pending'),
  (2, 1299.00, 'paid'),
  (3, 45.00, 'cancelled');

INSERT INTO order_items (order_id, product, quantity, unit_price) VALUES
  (1, '机械键盘', 1, 299.00),
  (2, '鼠标垫', 2, 44.75),
  (3, '显示器', 1, 1299.00),
  (4, 'USB-C 线', 1, 45.00);

CREATE VIEW active_users AS
SELECT u.id, u.name, u.email, COUNT(o.id) AS order_count
FROM users u
LEFT JOIN orders o ON o.user_id = u.id
GROUP BY u.id, u.name, u.email;
