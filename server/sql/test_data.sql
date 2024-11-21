-- Create test user
INSERT INTO users (uuid, username, salt, hashed_password, recovery_key) VALUES(
    '01934c200858726eb27c9912741d418e',
    'test',
    'fcd66c515a436c812a3230379edbbed7acfb1e67a036d58779641adec812a831',
    '15676f9b2df45f8afde42ffe4d7e1a5a5bc4341aae89742ed4dfca6b8c50d386',
    'e687bb9b11d3eab59830ce503ac62eaa6217554ec4aba2ac76fee93abd54685f'
);
