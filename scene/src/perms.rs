use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    comms::{PermsEvent, SceneEvent},
    Id,
};

pub const CANONICAL_UPDATER: Id = 0;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum Perm {
    /// Changes to the overall scene. Updating the map size, changing the
    /// layers, etc. Only editors may edit the scene.
    SceneEdit,

    /// Editing individual layers. Adding or removing sprites to or from a
    /// layer. Not changing layer visibility or lock status. Editors or players
    /// with permissions on a given layer may edit layers.
    LayerEdit,

    /// Moving sprites or changing their visuals. Not adding or removing
    /// sprites. Editors or players with permissions on a given sprite may edit
    /// sprites.
    SpriteEdit,

    /// Creating new drawings, adding points to drawings. Players or better may
    /// edit the scene's drawings.
    DrawingEdit,

    /// Changes to sprite groupings in the scene. Creation and deletion of
    /// groups. Players or better may edit selection groups.
    GroupEdit,

    /// Dummy and EventSet should not be handled directly. Dummy events should
    /// never make it over the wire and can be safely ignored in any case.
    /// EventSets should be unpacked and processed one by one.
    Special,
}

impl Perm {
    pub fn of(event: &SceneEvent) -> Perm {
        match *event {
            SceneEvent::FogActive(..)
            | SceneEvent::FogOcclude(..)
            | SceneEvent::FogReveal(..)
            | SceneEvent::LayerNew(..)
            | SceneEvent::LayerLocked(..)
            | SceneEvent::LayerMove(..)
            | SceneEvent::LayerRename(..)
            | SceneEvent::LayerVisibility(..)
            | SceneEvent::LayerRemove(..)
            | SceneEvent::LayerRestore(..)
            | SceneEvent::SpriteLayer(..)
            | SceneEvent::SceneDimensions(..)
            | SceneEvent::SceneTitle(..) => Perm::SceneEdit,
            SceneEvent::SpriteNew(..)
            | SceneEvent::SpriteRemove(..)
            | SceneEvent::SpriteRestore(..) => Perm::LayerEdit,
            SceneEvent::GroupAdd(..)
            | SceneEvent::GroupRemove(..)
            | SceneEvent::SpriteMove(..)
            | SceneEvent::SpriteVisual(..)
            | SceneEvent::SpriteDrawingFinish(..) => Perm::SpriteEdit,
            SceneEvent::SpriteDrawingStart(..) | SceneEvent::SpriteDrawingPoint(..) => {
                Perm::DrawingEdit
            }
            SceneEvent::GroupNew(..) | SceneEvent::GroupDelete(..) => Perm::GroupEdit,
            SceneEvent::Dummy | SceneEvent::EventSet(..) => Perm::Special,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Role {
    /// Cannot interact with sprites or layers.
    Spectator = 0,
    /// Can only handle sprites on specific layers.
    Player = 1,
    /// Full permissions to alter scene.
    Editor = 2,
    /// Full permissions, irrevocable.
    Owner = 3,
}

impl Role {
    fn allows(&self, perm: Perm) -> bool {
        match perm {
            Perm::SceneEdit => self.editor(),
            Perm::LayerEdit => self.editor(),
            Perm::SpriteEdit => self.editor(),
            Perm::DrawingEdit => self.player(),
            Perm::GroupEdit => self.player(),
            Perm::Special => false,
        }
    }

    fn lowest() -> Self {
        Role::Spectator
    }

    /// This role is a spectator or lower
    pub fn spectator(&self) -> bool {
        *self <= Self::Spectator
    }

    /// This role is a player or higher
    pub fn player(&self) -> bool {
        *self >= Self::Player
    }

    /// This role is an editor or higher
    pub fn editor(&self) -> bool {
        *self >= Self::Editor
    }
}

/// This user is granted certain permissions over a single item.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Override {
    user: Id,
    item: Id,
}

impl Override {
    fn allows(&self, user: Id, perm: Perm, sprite: Option<Id>, layer: Option<Id>) -> bool {
        perm == Perm::SpriteEdit
            && self.user == user
            && (Some(self.item) == sprite || Some(self.item) == layer)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Perms {
    roles: HashMap<Id, Role>,
    overrides: Vec<Override>,
}

impl Perms {
    pub fn new() -> Self {
        let mut roles = HashMap::new();
        roles.insert(CANONICAL_UPDATER, Role::Owner);
        Self {
            roles,
            overrides: Vec::new(),
        }
    }

    pub fn get_role(&self, user: Id) -> Role {
        *self.roles.get(&user).unwrap_or(&Role::lowest())
    }

    fn set_role(&mut self, user: Id, role: Role) {
        self.roles.insert(user, role);
    }

    fn allowed_by_role(&self, user: Id, perm: Perm) -> bool {
        self.get_role(user).allows(perm)
    }

    fn allowed_by_override(
        &self,
        user: Id,
        perm: Perm,
        sprite: Option<Id>,
        layer: Option<Id>,
    ) -> bool {
        self.overrides
            .iter()
            .any(|o| o.allows(user, perm, sprite, layer))
    }

    pub fn set_owner(&mut self, owner: Id) {
        self.roles.insert(owner, Role::Owner);
    }

    pub fn role_change(&mut self, updater: Id, user: Id, role: Role) -> Option<PermsEvent> {
        let updater_role = self.get_role(updater);
        let user_role = self.get_role(user);

        // The owner's role cannot be updated.
        // The role of the updater must exceed or equal both the new role and
        // the role recipient.
        if !matches!(user_role, Role::Owner)
            && !matches!(role, Role::Owner)
            && updater_role >= role
            && updater_role >= user_role
        {
            self.set_role(user, role);
            Some(PermsEvent::RoleChange(user, role))
        } else {
            None
        }
    }

    fn add_override(&mut self, new: Override) -> PermsEvent {
        if !self.overrides.contains(&new) {
            self.overrides.push(new.clone());
        }
        PermsEvent::NewOverride(new)
    }

    pub fn new_override(&mut self, updater: Id, new: Override) -> Option<PermsEvent> {
        if self.get_role(updater) >= Role::Editor {
            Some(self.add_override(new))
        } else {
            None
        }
    }

    pub fn handle_event(&mut self, updater: Id, event: PermsEvent) -> bool {
        match event {
            PermsEvent::RoleChange(user, role) => self.role_change(updater, user, role),
            PermsEvent::NewOverride(new) => self.new_override(updater, new),
        }
        .is_some()
    }

    pub fn selectable(&self, user: Id, sprite: Id, layer: Id) -> bool {
        if self.get_role(user).allows(Perm::SpriteEdit) {
            true
        } else {
            self.overrides
                .iter()
                .any(|o| o.allows(user, Perm::SpriteEdit, Some(sprite), Some(layer)))
        }
    }

    /// Check if a given event is permitted for this user. The optional layer
    /// parameter should have the ID of the layer that contains the relevant
    /// sprite for the event, if applicable.
    pub fn permitted(&self, user: Id, event: &SceneEvent, layer: Option<Id>) -> bool {
        if let SceneEvent::EventSet(events) = event {
            events.iter().all(|e| self.permitted(user, e, layer))
        } else {
            let perm = Perm::of(event);
            self.allowed_by_role(user, perm)
                || self.allowed_by_override(user, perm, event.sprite(), layer)
        }
    }

    /// Allow the creators of sprites or layers to update or delete them.
    pub fn created(&mut self, user: Id, event: &SceneEvent) -> Option<PermsEvent> {
        if matches!(
            event,
            SceneEvent::LayerNew(..)
                | SceneEvent::SpriteNew(..)
                | SceneEvent::SpriteDrawingStart(..)
        ) && let Some(item) = event.item()
        {
            Some(self.add_override(Override { user, item }))
        } else {
            None
        }
    }
}

impl Default for Perms {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{Rect, Sprite};

    #[test]
    fn test_role_precedence() {
        assert!(Role::Owner > Role::Editor);
        assert!(Role::Editor > Role::Player);
        assert!(Role::Player > Role::Spectator);
    }

    #[test]
    fn test_override_handling() {
        let user = 123;
        let layer = 1;
        let sprite = 2;
        let mut perms = Perms::new();

        // User is a player.
        perms.role_change(CANONICAL_UPDATER, user, Role::Player);

        // User should be granted permission over their layer.
        assert!(perms
            .created(user, &SceneEvent::LayerNew(layer, "user".into(), -1))
            .is_some());

        // User should not be able to create a sprite on a layer they don't own.
        assert!(!perms.permitted(
            user,
            &SceneEvent::SpriteNew(Sprite::new(4, None), 3),
            Some(3)
        ));

        // User should be able to create a sprite in their layer.
        let sprite_event = SceneEvent::SpriteNew(Sprite::new(sprite, None), layer);
        assert!(perms.permitted(user, &sprite_event, Some(layer)));

        // Creating the sprite, user should be granted ownership thereof.
        assert!(perms.created(user, &sprite_event).is_some());

        // User should be able to modify the sprite.
        assert!(perms.permitted(
            user,
            &SceneEvent::SpriteMove(sprite, Rect::new(1., 1., 1., 1.), Rect::new(0., 1., 1., 1.)),
            Some(layer)
        ));

        // User to be able to remove this sprite, or any sprite from their
        // layer, but not from other layers. Other users should not by default
        // be permitted to remove sprites from this users layer.
        assert!(perms.permitted(user, &SceneEvent::SpriteRemove(sprite, layer), Some(layer)));
        assert!(perms.permitted(user, &SceneEvent::SpriteRemove(5, layer), Some(layer)));
        assert!(!perms.permitted(user, &SceneEvent::SpriteRemove(6, 7), Some(7)));
        assert!(!perms.permitted(124, &SceneEvent::SpriteRemove(sprite, layer), Some(layer)));
    }

    #[test]
    fn test_drawing_handling() {
        let user = 1;
        let drawing = 2;
        let sprite = 3;
        let layer = 4;

        let mut perms = Perms::new();
        perms.role_change(CANONICAL_UPDATER, user, Role::Player);
        assert!(perms
            .created(user, &SceneEvent::LayerNew(layer, "layer".into(), -1))
            .is_some());
        assert!(perms
            .created(
                user,
                &SceneEvent::SpriteDrawingStart(drawing, crate::DrawingMode::Freehand)
            )
            .is_some());
        assert!(perms
            .created(
                user,
                &SceneEvent::SpriteNew(Sprite::new(sprite, None), layer)
            )
            .is_some());
        assert!(perms.permitted(
            user,
            &SceneEvent::SpriteDrawingPoint(drawing, crate::Point::same(1.)),
            None
        ));
        assert!(perms.permitted(
            user,
            &SceneEvent::SpriteDrawingFinish(drawing, sprite),
            Some(layer)
        ));
    }
}
