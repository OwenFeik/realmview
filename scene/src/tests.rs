use crate::{Point, Scene, SpriteDrawing, SpriteVisual};

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

#[test]
fn test_sprite_drawing() {
    let mut server = Scene::new();
    let mut client = server.non_canon();

    let event = client
        .new_sprite(
            Some(SpriteVisual::Drawing(SpriteDrawing::new())),
            client.first_layer(),
        )
        .unwrap();
    assert!(server.apply_event(event));

    let sprite_id = server.layer(server.first_layer()).unwrap().sprites[0].id;
    let event = client
        .sprite(sprite_id)
        .unwrap()
        .add_drawing_point(Point::same(1.0))
        .unwrap();

    assert!(server.apply_event(event));
    assert!(server.sprite(sprite_id).unwrap().n_drawing_points() == 2);
}
