use super::*;

/// Test that if there is a selectable sprite visible below a non-selectable
/// sprite, it's possible to click on that sprite.
#[test]
fn test_select_behind_forbidden() {
    let mut int = Interactor::new(None);
    int.user = 1;
    int.role = scene::perms::Role::Player;

    // New layer behind foreground.
    let Some(SceneEvent::LayerNew(layer, ..)) =
        int.scene.new_layer("player", Scene::FOREGROUND_Z - 1)
    else {
        panic!("Layer not created.");
    };
    int.perms.grant_override(int.user, layer);

    let server_sprite_id = 3;
    let visual = SpriteVisual::Shape {
        shape: Shape::Rectangle,
        stroke: 1.,
        solid: false,
        colour: scene::Colour::DEFAULT,
    };
    let mut server_sprite = Sprite::new(server_sprite_id, Some(visual.clone()));
    server_sprite.set_rect(Rect::new(-2., -2., 4., 4.)); // 4x4 covering the origin.

    int.process_server_event(ServerEvent::SceneUpdate(SceneEvent::SpriteNew(
        server_sprite,
        int.scene.first_layer(),
    )));

    let Some(SceneEvent::SpriteNew(player_sprite, _)) = int.scene.new_sprite(Some(visual), layer)
    else {
        panic!("Sprite not created.");
    };
    let player_sprite_id = player_sprite.id;

    assert_eq!(int.grab_at(Point::ORIGIN, false).1, Some(player_sprite_id));
}
