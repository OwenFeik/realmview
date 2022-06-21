use std::collections::HashMap;

use scene::{comms::SceneEvent, Id};

enum Permission {
    LayerManage,
    LayerMove,   
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Role {
    /// Full permissions to alter scene.
    Editor,

    /// Cannot interact with sprites or layers.
    Spectator,

    /// Can only handle sprites on specific layers.
    Player,
}

pub struct Perms {
    // Maps user ID to role
    roles: HashMap<i64, Role>,

    // Maps layer ID to users allowed to edit that layer
    layers: HashMap<Id, Vec<i64>>,

    // Maps sprite ID to users allowed to edit that sprite
    sprites: HashMap<Id, Vec<i64>>,
}

impl Perms {
    pub fn new() -> Self {
        Perms {
            roles: HashMap::new(),
            layers: HashMap::new(),
            sprites: HashMap::new(),
        }
    }

    fn get_role(&self, user: i64) -> Role {
        *self.roles.get(&user).unwrap_or(&Role::Spectator)
    }

    pub fn set_role(&mut self, user: i64, role: Role) {
        self.roles.insert(user, role);
    }

    pub fn permitted(&self, user: i64, event: &SceneEvent) -> bool {
        match self.get_role(user) {
            Role::Editor => true,
            Role::Player => match event {
                SceneEvent::SpriteMove(..) => true,
                _ => false
            },
            Role::Spectator => false
        }
    }
}
