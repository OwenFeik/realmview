use std::{collections::HashMap, fmt::Display};

use scene::Id;
use uuid::Uuid;

use crate::{
    scene::{
        comms::{PermsEvent, SceneEvent},
        perms::{self, Perms},
    },
    utils::{err, warning, Res},
};

#[derive(Clone, Hash, PartialEq, Eq)]
pub struct GameKey(String);

impl GameKey {
    const GAME_KEY_LENGTH: usize = 10;

    pub fn new() -> Res<Self> {
        crate::crypto::random_hex_string(Self::GAME_KEY_LENGTH).map(Self)
    }

    pub fn from<S: AsRef<str>>(key: S) -> Res<Self> {
        let string = key.as_ref().to_lowercase();
        if string.len() != Self::GAME_KEY_LENGTH {
            Err(format!(
                "Invalid game key. Game keys are {} characters long ({} provided).",
                Self::GAME_KEY_LENGTH,
                string.len()
            ))
        } else if !string
            .chars()
            .all(|c| c.is_numeric() || ('a'..='f').contains(&c))
        {
            err("Invalid game key. Game keys contain the characters 0-9 and a-f only.")
        } else {
            Ok(Self(string))
        }
    }
}

impl Display for GameKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", &self.0)
    }
}

pub struct Game {
    pub key: GameKey,
    project: scene::Project,
    scene: scene::Scene,
    perms: Perms,
    users: HashMap<Uuid, String>,
}

impl Game {
    pub fn new(project: scene::Project, scene: Uuid, owner: Uuid, key: GameKey) -> Self {
        let mut scene = project.get_scene(scene).unwrap().clone();
        scene.canon();
        let mut perms = Perms::new();
        perms.set_owner(owner);
        Self {
            project,
            key,
            scene,
            perms,
            users: HashMap::new(),
        }
    }

    pub fn copy_project(&self) -> scene::Project {
        self.project.clone()
    }

    pub fn project_uuid(&self) -> Uuid {
        self.scene.project
    }

    pub fn scene_uuid(&self) -> Uuid {
        self.scene.uuid
    }

    pub fn handle_perms(&mut self, user: Uuid, event: PermsEvent) -> bool {
        self.perms.handle_event(user, event)
    }

    pub fn owner_is(&self, user: Uuid) -> bool {
        matches!(self.perms.get_role(user), perms::Role::Owner)
    }

    /// Given a user ID and that users name, find a layer with that users name
    /// or create one and return it. If that user is the game owner, don't do
    /// this and just return (None, None).
    fn player_layer(&mut self, user: Uuid) -> (Option<SceneEvent>, Option<scene::Id>) {
        let Some(name) = self.users.get(&user) else {
            warning(format!(
                "(Game: {}) Couldn't find player (Id: {}) name.",
                &self.key, user
            ));
            return (None, None);
        };

        if self.perms.get_role(user) == perms::Role::Owner {
            return (None, None);
        }

        for layer in &self.scene.layers {
            if layer.title.eq(name) {
                return (None, Some(layer.id));
            }
        }

        if let Some(event) = self.scene.new_layer(name, scene::Scene::FOREGROUND_Z) {
            let id = if let &SceneEvent::LayerNew(id, ..) = &event {
                id
            } else {
                0
            };

            (Some(event), Some(id))
        } else {
            (None, None)
        }
    }

    /// Adds a player to the permissions set up and creates a layer for that
    /// player if none exists.
    ///
    /// Returns a tuple of (perms_event, scene_event, player_layer)
    pub fn add_player(
        &mut self,
        user: Uuid,
        name: &str,
    ) -> (Vec<PermsEvent>, Option<SceneEvent>, Option<scene::Id>) {
        self.users.insert(user, name.to_string());

        let mut perms = Vec::new();
        if let Some(event) =
            self.perms
                .role_change(perms::CANONICAL_UPDATER, user, perms::Role::Player)
        {
            perms.push(event);
        }

        let (scene, layer) = self.player_layer(user);
        if let Some(id) = layer
            && let Some(event) = self.perms.grant_override(user, id)
        {
            perms.push(event);
        }

        (perms, scene, layer)
    }

    /// Handle removal of a layer by recreating a player layer if needed.
    pub fn handle_remove_layer(
        &mut self,
        event: SceneEvent,
    ) -> Option<(Uuid, Id, Option<SceneEvent>)> {
        let SceneEvent::LayerRemove(layer) = event else {
            return None;
        };

        if let Some(layer) = self.scene.removed_layers.iter().find(|l| l.id == layer) {
            if let Some(user) = self
                .users
                .iter()
                .find(|(_, name)| name == &&layer.title)
                .map(|(user, _)| *user)
            {
                if let (event, Some(id)) = self.player_layer(user) {
                    return Some((user, id, event));
                }
            }
        }

        None
    }

    pub fn handle_event(&mut self, user: Uuid, event: SceneEvent) -> bool {
        let layer = self.scene.event_layer(&event);
        self.perms.permitted(user, &event, layer) && self.scene.apply_event(event.clone())
    }

    pub fn switch_to_scene(&mut self, scene: Uuid) -> Res<()> {
        let mut to_save = scene::Scene::default();
        std::mem::swap(&mut self.scene, &mut to_save);

        // If this doesn't work it's not really recoverable.
        self.project.update_scene(to_save).ok();

        if let Some(new_scene) = self.project.get_scene(scene).cloned() {
            self.scene = new_scene;
            Ok(())
        } else {
            err("Specified scene does not exist")
        }
    }

    pub fn server_scene(&self) -> scene::Scene {
        self.scene.clone()
    }

    pub fn client_scene(&mut self) -> scene::Scene {
        self.scene.non_canon()
    }

    pub fn scene_list(&self) -> (Vec<(String, Uuid)>, Uuid) {
        let list = self
            .project
            .scenes
            .iter()
            .map(|s| (s.title.clone(), s.uuid))
            .collect();
        (list, self.scene.uuid)
    }

    pub fn client_perms(&mut self) -> Perms {
        self.perms.clone()
    }
}

#[cfg(test)]
mod test {
    use scene::{comms::SceneEvent, Colour, Point, Project, Rect, Sprite, SpriteVisual};

    use super::Game;
    use crate::{games::game::GameKey, utils::generate_uuid};

    #[test]
    fn test_permissions() {
        let mut project = Project::new(generate_uuid());
        let scene = project.new_scene().uuid;
        let owner = generate_uuid();
        let owner_layer = 5;
        let player = generate_uuid();
        let mut game = Game::new(project, scene, owner, GameKey::new().unwrap());

        // Owner should be able to add a new layer and a sprite to that layer.
        let owner_sprite = 6;
        assert!(game.handle_event(owner, SceneEvent::LayerNew(owner_layer, "extra".into(), 1)));
        assert!(game.handle_event(
            owner,
            SceneEvent::SpriteNew(Sprite::new(owner_sprite, None), owner_layer)
        ));

        // Adding a new player should create a layer for that player.
        let (perms, layer_event, layer_opt) = game.add_player(player, "player");
        assert!(!perms.is_empty());
        assert!(layer_event.is_some());
        assert!(layer_opt.is_some());
        let player_layer = layer_opt.unwrap();

        // Player should be able to add a sprite to their own player, but not
        // the owner's layer.
        let player_sprite = 5;
        assert!(!game.handle_event(
            player,
            SceneEvent::SpriteNew(Sprite::new(player_sprite, None), owner_layer)
        ));
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteNew(Sprite::new(player_sprite, None), player_layer)
        ));

        // Player should be able to modify their sprite, but not the owner's.
        let from = game.scene.sprite(player_sprite).unwrap().rect;
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteMove(player_sprite, from, Rect::new(1., 1., 1., 1.))
        ));
        let from = game.scene.sprite(owner_sprite).unwrap().rect;
        assert!(!game.handle_event(
            player,
            SceneEvent::SpriteMove(owner_sprite, from, Rect::new(1., 1., 1., 1.))
        ));

        // Player should be able to remove their sprite, but not the owner's.
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteRemove(player_sprite, player_layer)
        ));
        assert!(!game.handle_event(player, SceneEvent::SpriteRemove(owner_sprite, owner_layer)));

        // Owner should be able to remove their sprite.
        assert!(game.handle_event(owner, SceneEvent::SpriteRemove(owner_sprite, owner_layer)));

        // All sprites should be removed.ABCDEFGH
        assert!(game.scene.sprite(player_sprite).is_none());
        assert!(game.scene.sprite(owner_sprite).is_none());
    }

    #[test]
    fn test_drawings() {
        let mut project = Project::new(generate_uuid());
        let scene = project.new_scene().uuid;
        let owner = generate_uuid();
        let drawing = 3;
        let player = generate_uuid();
        let sprite = 5;

        let mut game = Game::new(project, scene, owner, GameKey::new().unwrap());
        let (_, _, layer) = game.add_player(player, "player");
        let layer = layer.unwrap();

        assert!(game.handle_event(
            player,
            SceneEvent::SpriteDrawingStart(drawing, scene::DrawingMode::Freehand)
        ));
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteNew(
                Sprite::new(
                    sprite,
                    Some(SpriteVisual::Drawing {
                        drawing,
                        colour: Colour::DEFAULT,
                        stroke: 1.,
                        cap_start: scene::Cap::Arrow,
                        cap_end: scene::Cap::Round
                    })
                ),
                layer
            )
        ));
        assert!(game.handle_event(
            player,
            SceneEvent::SpriteDrawingPoint(drawing, Point::same(1.))
        ));
    }
}
