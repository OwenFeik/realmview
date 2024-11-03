use uuid::Uuid;

use self::details::SceneDetails;
use super::*;

fn generate_uuid() -> Uuid {
    Uuid::new_v7(uuid::Timestamp::now(uuid::NoContext))
}

fn add_player_layer(int: &mut Interactor, player: Uuid) -> Id {
    // New layer behind foreground.
    let Some(SceneEvent::LayerNew(layer, ..)) =
        int.scene.new_layer("player", Scene::FOREGROUND_Z - 1)
    else {
        panic!("Layer not created.");
    };
    int.perms.grant_override(player, layer);
    layer
}

/// Test that if there is a selectable sprite visible below a non-selectable
/// sprite, it's possible to click on that sprite.
#[test]
fn test_select_behind_forbidden() {
    let player = generate_uuid();

    let mut int = Interactor::new(None);
    int.user = player;
    int.role = scene::perms::Role::Player;

    let layer = add_player_layer(&mut int, player);

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

/// Test that a game owner can select a sprite created by another player.
#[test]
fn test_select_player_sprite() {
    let owner = 1;
    let player = 2;

    let mut int = Interactor::new(None);
    int.perms.set_owner(owner);
    int.user = owner;
    int.role = scene::perms::Role::Owner;

    let layer = add_player_layer(&mut int, player);
    let sprite = layer + 1;

    int.process_server_event(ServerEvent::SceneUpdate(SceneEvent::SpriteNew(
        Sprite::new(sprite, None),
        layer,
    )));
    let sprite = int.scene.sprite_ref(sprite).unwrap();

    assert!(int.perms.selectable(owner, sprite.id, layer));
    assert!(int.selectable(sprite, true));
}

#[test]
fn test_fog_active_triggers_scene() {
    let mut int = Interactor::new(None);
    int.scene_details(SceneDetails {
        fog: Some(false),
        ..Default::default()
    });
    int.scene_details(SceneDetails {
        fog: Some(true),
        ..Default::default()
    });
    assert!(int.save_required());
}
