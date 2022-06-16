use crate::{comms::SceneEventAck, Scene, Sprite};

#[test]
fn test_add_sprite() {
    let mut scene = Scene::new();
    scene.canon(); // Make scene canonical so events will be issued.

    let sprite = Sprite::new(1, scene.layers[0].local_id);
    let event = scene.add_sprite(sprite).unwrap();
    assert!(!scene.sprites.is_empty());
    scene.unwind_event(event);
    assert!(scene.sprites.is_empty());
}

#[test]
fn test_layer_move() {
    let mut scene = Scene::new();

    let layer_zs = |s: &Scene| s.layers.iter().map(|l| l.z).collect();

    let starting_zs: Vec<i32> = layer_zs(&scene);

    // Set canonical ID for top layer so event will be issued.
    let top_layer = scene.layers[0].local_id;
    scene.apply_ack(&SceneEventAck::LayerNew(top_layer, Some(1)));

    let event = scene.move_layer(top_layer, false).unwrap();
    assert_eq!(vec![-1, -2, -3], layer_zs(&scene));

    scene.unwind_event(event);
    assert_eq!(starting_zs, layer_zs(&scene));
}
