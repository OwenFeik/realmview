use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    comms::{PermsEvent, SceneEvent},
    Id,
};

pub const CANONICAL_UPDATER: Id = 0;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum Perm {
    FogEdit,
    LayerNew,
    LayerRemove,
    LayerUpdate,
    SceneDetails,
    Special,
    SpriteNew,
    SpriteRemove,
    SpriteUpdate,
}

impl Perm {
    pub fn of(event: &SceneEvent) -> Perm {
        match *event {
            SceneEvent::Dummy | SceneEvent::EventSet(..) => Perm::Special,
            SceneEvent::FogActive(..) | SceneEvent::FogOcclude(..) | SceneEvent::FogReveal(..) => {
                Perm::FogEdit
            }
            SceneEvent::LayerLocked(..)
            | SceneEvent::LayerMove(..)
            | SceneEvent::LayerRename(..)
            | SceneEvent::LayerVisibility(..) => Perm::LayerUpdate,
            SceneEvent::LayerRemove(..) => Perm::LayerRemove,
            SceneEvent::LayerNew(..) | SceneEvent::LayerRestore(..) => Perm::LayerNew,
            SceneEvent::SceneDimensions(..) | SceneEvent::SceneTitle(..) => Perm::SceneDetails,
            SceneEvent::SpriteLayer(..) => Perm::LayerUpdate,
            SceneEvent::GroupNew(..)
            | SceneEvent::GroupAdd(..)
            | SceneEvent::GroupDelete(..)
            | SceneEvent::GroupRemove(..)
            | SceneEvent::SpriteMove(..)
            | SceneEvent::SpriteVisual(..)
            | SceneEvent::SpriteDrawingStart(..)
            | SceneEvent::SpriteDrawingFinish(..)
            | SceneEvent::SpriteDrawingPoint(..) => Perm::SpriteUpdate,
            SceneEvent::SpriteNew(..) | SceneEvent::SpriteRestore(..) => Perm::SpriteNew,
            SceneEvent::SpriteRemove(..) => Perm::SpriteRemove,
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
        if self >= &Role::Editor {
            return true;
        }

        match perm {
            Perm::Special => false,
            _ => self.editor(),
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
        self.user == user
            && match perm {
                Perm::FogEdit => false,
                Perm::LayerNew => false,
                Perm::LayerRemove => false,
                Perm::LayerUpdate => false,
                Perm::SceneDetails => false,
                Perm::Special => false,
                Perm::SpriteNew | Perm::SpriteRemove => layer == Some(self.item),
                Perm::SpriteUpdate => sprite == Some(self.item),
            }
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

    fn allowed_by_role(&self, user: Id, event: &SceneEvent) -> bool {
        self.get_role(user).allows(Perm::of(event))
    }

    fn allowed_by_override(&self, user: Id, event: &SceneEvent) -> bool {
        let perm = Perm::of(event);
        let sprite = event.sprite();
        let layer = event.layer();
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
        const REQUIRED_PERM: Perm = Perm::SpriteUpdate;

        self.get_role(user).allows(REQUIRED_PERM)
            || self
                .overrides
                .iter()
                .any(|o| o.allows(user, REQUIRED_PERM, Some(sprite), Some(layer)))
    }

    pub fn permitted(&self, user: Id, event: &SceneEvent) -> bool {
        if let SceneEvent::EventSet(events) = event {
            events.iter().all(|e| self.permitted(user, e))
        } else {
            self.allowed_by_role(user, event) || self.allowed_by_override(user, event)
        }
    }

    /// Allow the creators of sprites or layers to update or delete them.
    pub fn ownership_override(&mut self, user: Id, event: &SceneEvent) -> Option<PermsEvent> {
        if matches!(event, SceneEvent::LayerNew(..) | SceneEvent::SpriteNew(..))
            && let Some(item) = event.item()
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
            .ownership_override(user, &SceneEvent::LayerNew(layer, "user".into(), -1))
            .is_some());

        // User should not be able to create a sprite on a layer they don't own.
        assert!(!perms.permitted(user, &SceneEvent::SpriteNew(Sprite::new(4, None), 3)));

        // User should be able to create a sprite in their layer.
        let sprite_event = SceneEvent::SpriteNew(Sprite::new(sprite, None), layer);
        assert!(perms.permitted(user, &sprite_event));

        // Creating the sprite, user should be granted ownership thereof.
        assert!(perms.ownership_override(user, &sprite_event).is_some());

        // User should be able to modify the sprite.
        assert!(perms.permitted(
            user,
            &SceneEvent::SpriteMove(sprite, Rect::new(1., 1., 1., 1.), Rect::new(0., 1., 1., 1.))
        ));

        // User to be able to remove this sprite, or any sprite from their
        // layer, but not from other layers.
        assert!(perms.permitted(user, &SceneEvent::SpriteRemove(sprite, layer)));
        assert!(perms.permitted(user, &SceneEvent::SpriteRemove(5, layer)));
        assert!(!perms.permitted(user, &SceneEvent::SpriteRemove(6, 7)));
    }
}
