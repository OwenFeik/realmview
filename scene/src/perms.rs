use std::collections::HashMap;

use crate::{comms::SceneEvent, Id};

#[derive(PartialEq, Eq)]
enum Perm {
    LayerNew,
    LayerRemove,
    LayerUpdate,
    Special,
    SpriteNew,
    SpriteRemove,
    SpriteUpdate,
}

impl Perm {
    pub fn of(event: &SceneEvent) -> Perm {
        match *event {
            SceneEvent::Dummy
                | SceneEvent::EventSet(..) => Perm::Special,
            SceneEvent::LayerLocked(..)
                | SceneEvent::LayerMove(..)
                | SceneEvent::LayerRename(..)
                | SceneEvent::LayerVisibility(..) => Perm::LayerUpdate,
            SceneEvent::LayerRemove(..) => Perm::LayerRemove,
            SceneEvent::LayerNew(..)
                | SceneEvent::LayerRestore(..) => Perm::LayerNew,
            SceneEvent::SpriteLayer(..) => Perm::LayerUpdate,
            SceneEvent::SpriteMove(..)
                | SceneEvent::SpriteTexture(..) => Perm::SpriteUpdate,
            SceneEvent::SpriteNew(..)
                | SceneEvent::SpriteRestore(..) => Perm::SpriteNew,
            SceneEvent::SpriteRemove(..) => Perm::SpriteRemove
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Full permissions, irrevocable.
    Owner,

    /// Full permissions to alter scene.
    Editor,

    /// Can only handle sprites on specific layers.
    Player,

    /// Cannot interact with sprites or layers.
    Spectator,
}

impl Role {
    fn allows(&self, perm: Perm) -> bool {
        match perm {
            Perm::Special => false,
            Perm::SpriteUpdate => !matches!(self, Self::Spectator),
            _ => matches!(self, Self::Editor),
        }
    }

    fn lowest() -> Self {
        Role::Spectator
    }
}

struct PermSet {
    item: Id,
    users: Vec<Id>,
    role: Role 
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

struct Override {
    user: Id,
    perm: Perm,
    item: Option<Id>,
}

impl Override {
    fn allows(&self, user: Id, event: &SceneEvent) -> bool {
        if user != self.user 
            || Perm::of(event) != self.perm
            || self.item.is_some() && event.item() != self.item
        {
            false
        }
        else {
            true
        }
    }
}

pub struct Permissions {
    roles: HashMap<Id, Role>,
    layers: HashMap<Id, PermSet>,
    sprites: HashMap<Id, PermSet>,
    overrides: Vec<Override>,
}

impl Permissions {
    pub fn new() -> Self {
        Self {
            roles: HashMap::new(),
            layers: HashMap::new(),
            sprites: HashMap::new(),
            overrides: Vec::new(),
        }
    }

    fn get_role(&self, user: Id) -> Role {
        *self.roles.get(&user).unwrap_or(&Role::lowest())
    }

    fn set_role(&mut self, user: Id, role: Role) {
        self.roles.insert(user, role);
    }

    fn allowed_by_role(&self, user: Id, event: &SceneEvent, layer: Option<Id>) -> bool {
        let role = self.get_role(user);
        if let Some(id) = layer {
            if let Some(ps) = self.layers.get(&id) {
                if !ps.allows(user, role) {
                    return false;
                }
            } 
        }

        if event.is_sprite() {
            if let Some(ps) = self.layers.get(&event.item().unwrap()) {
                if !ps.allows(user, role) {
                    return false;
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

    pub fn role_change(&mut self, updater: Id, user: Id, role: Role) -> bool {
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
            true
        }
        else {
            false
        }
    }

    pub fn permitted(&self, user: Id, event: &SceneEvent, layer: Option<Id>) -> bool {
        self.allowed_by_role(user, event, layer) || self.allowed_by_override(user, event)
    }
}
