PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL,
    salt CHAR(64) NOT NULL,
    hashed_password CHAR(64) NOT NULL,
    recovery_key CHAR(64) NOT NULL,
    created_time INTEGER DEFAULT strftime('%s', CURRENT_TIMESTAMP) NOT NULL
);

CREATE TABLE IF NOT EXISTS user_sessions (
    id INTEGER PRIMARY KEY,
    user INTEGER REFERENCES users(id) NOT NULL,
    session_key CHAR(64) NOT NULL,
    active BOOLEAN DEFAULT TRUE NOT NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);
