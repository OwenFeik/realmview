use crate::Scene;

#[test]
fn test_layer_move() {
    let mut scene = Scene::new();

    let layer_zs = |s: &Scene| s.layers.iter().map(|l| l.z).collect();

    let starting_zs: Vec<i32> = layer_zs(&scene);

    let top_layer = scene.layers[0].id;

    let event = scene.move_layer(top_layer, false).unwrap();
    assert_eq!(vec![-1, -2, -3], layer_zs(&scene));

    scene.unwind_event(event);
    assert_eq!(starting_zs, layer_zs(&scene));
}
