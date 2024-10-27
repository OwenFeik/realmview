PRAGMA foreign_keys = ON;

CREATE TABLE IF NOT EXISTS users (
    id INTEGER PRIMARY KEY,
    username TEXT NOT NULL UNIQUE,
    salt CHAR(64) NOT NULL,
    hashed_password CHAR(64) NOT NULL,
    recovery_key CHAR(64) NOT NULL,
    created_time INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS user_sessions (
    id INTEGER PRIMARY KEY,
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    session_key CHAR(64) NOT NULL UNIQUE,
    start_time INTEGER NOT NULL,
    end_time INTEGER
);

CREATE TABLE IF NOT EXISTS media (
    id INTEGER PRIMARY KEY,
    media_key CHAR(16) NOT NULL UNIQUE,
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    relative_path TEXT NOT NULL UNIQUE,
    title TEXT NOT NULL,
    hashed_value CHAR(64) NOT NULL,
    size INTEGER NOT NULL, -- Size in bytes
    w REAL NOT NULL, -- Default width of tokens created with this media
    h REAL NOT NULL, -- Default height of tokens created with this media
    UNIQUE(user, hashed_value)
);

CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY,
    project_key CHAR(16) NOT NULL, -- Used to name files for this project.
    user INTEGER REFERENCES users(id) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT,
    UNIQUE(project_key, user) -- Keys may repeat across users.
);

CREATE TABLE IF NOT EXISTS scenes (
    id INTEGER,
    scene_key CHAR(16) NOT NULL,
    project INTEGER REFERENCES projects(id) ON DELETE CASCADE NOT NULL,
    updated_time INTEGER NOT NULL,
    title TEXT,
    thumbnail TEXT,
    UNIQUE(scene_key, project) -- Keys may repeat across projects.
);
