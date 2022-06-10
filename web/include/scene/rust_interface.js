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

    scene_layers: missing_func,
    /*
    function scene_layers(): JsLayerInfo[]
    
    Returns array with a client::bridge::JsLayerInfo (exported type) struct for
    each layer in the scene.
    */

    new_sprite: missing_func,
    /*
    function new_sprite(texture_id: number)

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
};

// The below functions are used as externs in bridge.rs
// Begin :: Externs

// Allow exposure of closures with references to the relevant structs.
function expose_closure(name, closure) {
    RustFuncs[name] = closure;

    // When we get the scene_layers closure, we'll take that opportunity to
    // load the current layers in the scene.
    if (name == "scene_layers") {
        load_layers();
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
function load_texture(texture_id) {
    media_manager.load_media_with_id(texture_id, i => texture_queue.push(i));
}

// End :: Externs

// Wrapper around the exported closure that also reloads the layer list to keep
// it reflective of the new scene.
function load_scene(scene_encoded) {
    RustFuncs.load_scene(scene_encoded);
    load_layers();
}

// Wrapper that reloads layer list as well.
function new_scene(project_id = 0) {
    RustFuncs.new_scene(project_id);
    load_layers();
}

// Given an HTML image, load the texture for this image and add a sprite with
// that image to the scene.
function add_to_scene(image) {
    texture_queue.push(image);
    RustFuncs.new_sprite(
        parseInt(image.getAttribute("{{ constant(DATA_ID_ATTR) }}"))
    );
}

function rename_layer(layer_id, new_title) {
    RustFuncs.rename_layer(layer_id, new_title);
    load_layers();
}
