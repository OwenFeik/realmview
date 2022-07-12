#[test]
fn test_media_key_conversions() {
    let key = "0EB6AAF4DEB22C39";
    let id = super::Media::key_to_id(key).unwrap();
    assert_eq!(super::Media::id_to_key(id), key);

    let id = -1234567890;
    let key = super::Media::id_to_key(id);
    assert_eq!(super::Media::key_to_id(&key).unwrap(), id);
}
