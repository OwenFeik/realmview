use crate::{Point, Scene, SpriteVisual};

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

    let (drawing, event) = client.start_drawing(crate::DrawingMode::Freehand, Point::ORIGIN);
    assert!(server.apply_event(event.clone()));

    let event = client
        .new_sprite(
            Some(SpriteVisual::Drawing {
                drawing,
                colour: crate::Colour([0.0, 255.0, 0.0, 255.0]),
                stroke: crate::Sprite::DEFAULT_STROKE,
                cap_start: crate::Cap::Arrow,
                cap_end: crate::Cap::Round,
            }),
            client.first_layer(),
        )
        .unwrap();
    assert!(server.apply_event(event));

    let event = client.add_drawing_point(drawing, Point::same(1.0)).unwrap();
    assert!(server.apply_event(event));
    assert!(server.get_drawing(drawing).unwrap().last_point().unwrap() == Point::same(1.0));
}
