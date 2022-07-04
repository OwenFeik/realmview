const missing_func = (...args) => console.error(
    "Missing RustFunc. Arguments: " + args
);

// These functions are populated by the WASM app, in start.rs.
var RustFuncs = {
    export_scene: missing_func,
    /*
    function export_scene(): string

    Returns the base64 serde encoded form of the current scene.
    */
    
    load_scene: missing_func,
    /*
    function load_scene(scene_encoded: string)

    Given a base64 serde encoded scene, parses and sets as active this scene,
    overwriting the current scene.
    */
    
    new_scene: missing_func,
    /*
    function new_scene(project_id: number)

    If the current scene is an existing scene (has an ID set), replaces it with
    an empty scene with project ID as specified. Use project ID 0 for a new
    project.
    */

    new_sprite: missing_func,
    /*
    function new_sprite(layer_id: number, media_key: string)

    Adds a new sprite with the provided texture to the scene. Will load the
    texture if necessary.
    */

    rename_layer: missing_func,
    /*
    function rename_layer(layer_id: number, new_title: string)
    
    Renames the layer with local ID layer_id to the provided title.
    */

    layer_visible: missing_func,
    /*
    function layer_visible(layer_id: number, visible: bool)

    Sets visibility for the specified layer.
    */

    layer_locked: missing_func,
    /*
    function layer_locked(layer_id: number, locked: bool)

    Sets locked status for the specified layer.
    */

    new_layer: missing_func,
    /*
    function new_layer()

    Creates a new untitled layer at the top of the scene.
    */

    remove_layer: missing_func,
    /*
    function remove_layer(layer_id: number)

    Removes the specified layer from the scene.
    */

    sprite_layer: missing_func,
    /*
    function sprite_layer(sprite_id: number, layer_id: number)

    Moves the specified sprite to the specified layer.
    */

    sprite_details: missing_func,
    /*
    function sprite_details(sprite_id: number, json: string)

    Given a sprite and a JSON object, update the sprites attributes using the
    non-null dimensions of the json.
    */

    clone_sprite: missing_func,
    /*
    function clone_sprite(sprite_id: number)

    Clones the specified sprite.
    */

    remove_sprite: missing_func,
    /*
    function remove_sprite(sprite_id: number)

    Deletes the specified sprite.
    */
};

// Array of callbacks to be performed when a given closure is available.
const queued = {};

// The below functions are used as externs in bridge.rs
// Begin :: Externs

// Allow exposure of closures with references to the relevant structs.
function expose_closure(name, closure) {
    RustFuncs[name] = closure;

    if (queued[name]) {
        queued[name].forEach(cb => cb());
        delete queued[name];
    }
}

// Queue used to pass HTML images in to the TextureManager in program.rs.
const texture_queue = [];
function get_texture_queue() {
    return texture_queue;
}

// Loads the texture with the specified ID and pushes the HTML image to the
// texture queue. If necessary, this will query the desired image URL from
// the server.
function load_texture(media_key) {
    media_manager.load_media_with_key(media_key, i => texture_queue.push(i));
}

// scene/menu/layers/canvas_dropdown.html
// function sprite_downdown(id: number, x: number, y: number)

// scene/menu/sprite/sprite_menu.html
// function set_selected_sprite(sprite_json: string)
// function clear_selected_sprite()

// scene/game/role.js
// function update_interface(role: number)

// End :: Externs

// Queue a function to be called when closure func_name is loaded.
function call_when_ready(func_name, callback) {
    if (RustFuncs[func_name] != missing_func) {
        callback();
    }

    if (queued[func_name] === undefined) {
        queued[func_name] = [];
    } 

    queued[func_name].push(callback);
}

// Wrapper around the exported closure that also reloads the layer list to keep
// it reflective of the new scene.
function load_scene(scene_encoded) {
    call_when_ready("load_scene", () => {
        RustFuncs.load_scene(scene_encoded)
    });
}

// Wrapper that reloads layer list as well.
function new_scene(project_id = 0) {
    call_when_ready("new_scene", () => {
        RustFuncs.new_scene(project_id);
    });
    
}

// Given an HTML image, load the texture for this image and add a sprite with
// that image to the scene.
function add_to_scene(image) {
    texture_queue.push(image);
    call_when_ready("new_sprite", () => RustFuncs.new_sprite(
        selected_layer(),
        image.getAttribute("data-key")
    ));
}

function rename_layer(layer_id, new_title) {
    call_when_ready(
        "rename_layer",
        () => {
            RustFuncs.rename_layer(layer_id, new_title);
        }
    );
}

function new_layer() {
    call_when_ready(
        "new_layer",
        () => {
            RustFuncs.new_layer();
        }
    );
}
