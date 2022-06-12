use crate::{comms::SceneEventAck, Scene};

#[test]
fn test_layer_move() {
    let mut scene = Scene::new();

    let layer_zs = |s: &Scene| s.layers.iter().map(|l| l.z).collect();

    let starting_zs: Vec<i32> = layer_zs(&scene);

    // Set canonical ID for top layer so event will be issued.
    let top_layer = scene.layer(0).unwrap().local_id;
    scene.apply_ack(&SceneEventAck::LayerNew(top_layer, Some(1)));

    let event = scene.lower_layer(top_layer).unwrap();
    assert_eq!(vec![-1, -2, -3], layer_zs(&scene));

    scene.unwind_event(event);
    assert_eq!(starting_zs, layer_zs(&scene));
}
