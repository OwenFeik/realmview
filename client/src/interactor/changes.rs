pub struct Changes {
    // A change to a layer locked status, title, visibility, etc that will
    // require the layers list to be updated.
    layer: bool,

    // A change to the client role will require menu changes and a re-render.
    role: bool,

    // A change to a sprite that will require a re-render
    sprite: bool,

    // A change to the selected sprite that will require the sprite menu to be
    // updated.
    selected: bool,

    // A change to the scene list or active scene.
    scene: bool,
}

impl Changes {
    pub fn new() -> Self {
        Changes {
            layer: true,
            role: true,
            sprite: true,
            selected: true,
            scene: true,
        }
    }

    pub fn all_change(&mut self) {
        self.layer = true;
        self.sprite = true;
        self.selected = true;
        self.scene = true;
    }

    fn all_change_if(&mut self, changed: bool) {
        self.layer_change_if(changed);
        self.sprite_change_if(changed);
        self.selected_change_if(changed);
        self.scene = self.scene || changed;
    }

    fn layer_change(&mut self) {
        self.layer = true;
    }

    pub fn layer_change_if(&mut self, changed: bool) {
        self.layer = self.layer || changed;
    }

    pub fn handle_layer_change(&mut self) -> bool {
        let ret = self.layer;
        self.layer = false;
        ret
    }

    pub fn role_change(&mut self) {
        self.role = true;
    }

    pub fn role_change_if(&mut self, changed: bool) {
        self.role = self.role || changed;
    }

    pub fn handle_role_change(&mut self) -> bool {
        let ret = self.role;
        self.role = false;
        ret
    }

    pub fn sprite_change(&mut self) {
        self.sprite = true;
    }

    pub fn sprite_change_if(&mut self, changed: bool) {
        self.sprite = self.sprite || changed;
    }

    pub fn handle_sprite_change(&mut self) -> bool {
        let ret = self.sprite;
        self.sprite = false;
        ret
    }

    fn selected_change(&mut self) {
        self.selected = true;
    }

    pub fn selected_change_if(&mut self, changed: bool) {
        self.selected = self.selected || changed;
    }

    pub fn handle_selected_change(&mut self) -> bool {
        let ret = self.selected;
        self.selected = false;
        ret
    }

    pub fn sprite_selected_change(&mut self) {
        self.sprite = true;
        self.selected = true;
    }

    pub fn scene_change(&mut self) {
        self.scene = true;
    }

    pub fn scene_change_if(&mut self, changed: bool) {
        self.scene = self.scene || changed;
    }

    pub fn handle_scene_change(&mut self) -> bool {
        let ret = self.scene;
        self.scene = false;
        ret
    }
}
