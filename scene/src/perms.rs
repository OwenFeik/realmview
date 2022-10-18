use std::collections::HashMap;

use serde_derive::{Deserialize, Serialize};

use crate::{
    comms::{PermsEvent, SceneEvent},
    Id,
};

pub const CANONICAL_UPDATER: Id = 0;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
enum Perm {
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
            SceneEvent::LayerLocked(..)
            | SceneEvent::LayerMove(..)
            | SceneEvent::LayerRename(..)
            | SceneEvent::LayerVisibility(..) => Perm::LayerUpdate,
            SceneEvent::LayerRemove(..) => Perm::LayerRemove,
            SceneEvent::LayerNew(..) | SceneEvent::LayerRestore(..) => Perm::LayerNew,
            SceneEvent::SceneDimensions(..) | SceneEvent::SceneTitle(..) => Perm::SceneDetails,
            SceneEvent::SpriteLayer(..) => Perm::LayerUpdate,
            SceneEvent::SpriteMove(..)
            | SceneEvent::SpriteVisual(..)
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
            Perm::SpriteUpdate | Perm::SpriteNew => !self.spectator(),
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

/// For this item, only uses who are listed or have a role exceeding this role
/// may interact.
#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct PermSet {
    item: Id,
    users: Vec<Id>,
    role: Role,
}

impl PermSet {
    fn new(id: Id) -> Self {
        PermSet {
            item: id,
            users: vec![],
            role: Role::Editor,
        }
    }

    /// Whether this user is allowed to interact with this item
    fn allows(&self, user: Id, role: Role) -> bool {
        role >= self.role || self.users.contains(&user)
    }
}

/// This user is granted this permission, optionally over a single item.
#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct Override {
    user: Id,
    perm: Perm,
    item: Option<Id>,
}

impl Override {
    fn allows(&self, user: Id, event: &SceneEvent) -> bool {
        user == self.user
            && Perm::of(event) == self.perm
            && (self.item.is_none() || event.item() == self.item)
    }
}

#[derive(Clone, Deserialize, Serialize)]
pub struct Perms {
    roles: HashMap<Id, Role>,
    items: HashMap<Id, PermSet>,
    overrides: Vec<Override>,
}

impl Perms {
    pub fn new() -> Self {
        let mut roles = HashMap::new();
        roles.insert(CANONICAL_UPDATER, Role::Owner);
        Self {
            roles,
            items: HashMap::new(),
            overrides: Vec::new(),
        }
    }

    pub fn get_role(&self, user: Id) -> Role {
        *self.roles.get(&user).unwrap_or(&Role::lowest())
    }

    fn set_role(&mut self, user: Id, role: Role) {
        self.roles.insert(user, role);
    }

    fn allowed_by_role(&self, user: Id, event: &SceneEvent, layer: Option<Id>) -> bool {
        let role = self.get_role(user);
        if let Some(id) = layer {
            if let Some(ps) = self.items.get(&id) {
                if !ps.allows(user, role) {
                    return false;
                }
            }
        }

        if event.is_sprite() {
            if let Some(id) = event.item() {
                if let Some(ps) = self.items.get(&id) {
                    if !ps.allows(user, role) {
                        return false;
                    }
                }
            }
        }

        role.allows(Perm::of(event))
    }

    fn allowed_by_override(&self, user: Id, event: &SceneEvent) -> bool {
        self.overrides.iter().any(|o| o.allows(user, event))
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

    pub fn item_perms(&mut self, updater: Id, perms: PermSet) -> Option<PermsEvent> {
        if self.get_role(updater) >= Role::Editor {
            self.items.insert(perms.item, perms.clone());
            Some(PermsEvent::ItemPerms(perms))
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
            PermsEvent::ItemPerms(perms) => self.item_perms(updater, perms),
            PermsEvent::NewOverride(new) => self.new_override(updater, new),
        }
        .is_some()
    }

    pub fn permitted(&self, user: Id, event: &SceneEvent, layer: Option<Id>) -> bool {
        let pd = self.allowed_by_role(user, event, layer) || self.allowed_by_override(user, event);
        if !pd {
            println!("{event:?}");
        }
        pd
    }

    /// Allow the creators of sprites or layers to update or delete them.
    pub fn ownership_overrides(&mut self, user: Id, event: &SceneEvent) -> Option<Vec<PermsEvent>> {
        match event {
            SceneEvent::LayerNew(id, ..) => Some(vec![
                self.add_override(Override {
                    user,
                    perm: Perm::LayerUpdate,
                    item: Some(*id),
                }),
                self.add_override(Override {
                    user,
                    perm: Perm::LayerRemove,
                    item: Some(*id),
                }),
            ]),
            SceneEvent::SpriteNew(s, ..) => Some(vec![
                self.add_override(Override {
                    user,
                    perm: Perm::SpriteUpdate,
                    item: Some(s.id),
                }),
                self.add_override(Override {
                    user,
                    perm: Perm::SpriteRemove,
                    item: Some(s.id),
                }),
            ]),
            _ => None,
        }
    }
}

impl Default for Perms {
    fn default() -> Self {
        Self::new()
    }
}
