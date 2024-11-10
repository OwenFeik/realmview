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
    function new_scene()

    Create a new scene in the current project.
    */

    new_sprite: missing_func,
    /*
    function new_sprite(w: float, h: float, media_key: string)

    Adds a new sprite with the provided texture to the scene. Will load the
    texture if necessary.
    */

    set_scene_list: missing_func,
    /*
    function set_scene_list(scene_list_json: string)

    Sets the list of available scenes, input format [[Title, Key]]
    */
};

// Array of callbacks to be performed when a given closure is available.
const queued = {};

// The below functions are used as externs in bridge.rs
// Begin :: Externs

// Allow exposure of closures with references to the relevant structs.
function expose_closure(name, closure) {
    if (RustFuncs[name] !== missing_func && RustFuncs[name] !== undefined) {
        console.error(`Attempted to rebind exposed closure "${name}".`);
        return;
    }

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

// Return a canvas context with preserveDrawingBuffer, in order to enable
// screenshotting.
function get_webgl2_context(canvas) {
    return canvas.getContext('webgl2', { preserveDrawingBuffer: true });
}

// Loads the texture with the specified ID and pushes the HTML image to the
// texture queue. If necessary, this will query the desired image URL from
// the server.
function load_texture(media_key) {
    media_manager.load_media_with_key(media_key, i => texture_queue.push(i));
}

// scene/menu/sprite/sprite_menu.html
// function set_selected_sprite(sprite_json: string)
// function clear_selected_sprite()

// scene/project/project.js
// function set_active_scene(scene_key: string)

// /scene/project/project.js
// function upload_thumbnail()

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
function new_scene() {
    call_when_ready("new_scene", () => {
        RustFuncs.new_scene();
    });
}

// Given an HTML image, load the texture for this image and add a sprite with
// that image to the scene.
function add_to_scene(image) {
    texture_queue.push(image);
    call_when_ready("new_sprite", () => RustFuncs.new_sprite(
        parseFloat(image.getAttribute("data-w")) || 1.0,
        parseFloat(image.getAttribute("data-h")) || 1.0,
        image.getAttribute("data-media_key"),
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
