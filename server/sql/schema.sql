PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    uuid TEXT PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    salt CHAR(64) NOT NULL,
    hashed_password CHAR(64) NOT NULL,
    recovery_key CHAR(64) NOT NULL,
    created_time INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS user_sessions (
    uuid TEXT PRIMARY KEY,
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);

CREATE TABLE IF NOT EXISTS media (
    uuid TEXT PRIMARY KEY,
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    hashed_value CHAR(64) NOT NULL,
    file_size INTEGER NOT NULL, -- Size in bytes
    w REAL NOT NULL, -- Default width of tokens created with this media
    h REAL NOT NULL, -- Default height of tokens created with this media
    UNIQUE(user, hashed_value)
);

CREATE TABLE IF NOT EXISTS projects (
    uuid TEXT PRIMARY KEY,
    user TEXT REFERENCES users(uuid) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT
);

CREATE TABLE IF NOT EXISTS scenes (
    uuid TEXT PRIMARY KEY,
    project TEXT REFERENCES projects(uuid) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT,
    thumbnail TEXT
);
