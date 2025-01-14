const missing_func = (...args) => console.error(
    "Missing RustFunc. Arguments: " + args
);

// These functions are populated by the WASM app, in start.rs.
var RustFuncs = {
    new_sprite: missing_func,
    /*
    function new_sprite(w: float, h: float, media_key: string)

    Adds a new sprite with the provided texture to the scene. Will load the
    texture if necessary.
    */

    active_scene: missing_func
    /**
     * function active_scene(): string
     * 
     * Returns the UUID of the currently active scene.
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

// Uploads the currently visible canvas area to the server as a 256x196 png as
// a thumbnail for the specified scene.
function upload_thumbnail(scene_uuid) {
    const ASPECT = 4 / 3;
    const WIDTH = 256;
    const HEIGHT = WIDTH / ASPECT;

    // Find the largest rectangle with ASPECT ratio from the top left corner.  
    let canvas = document.getElementById("canvas");
    let sw, sh;
    if (canvas.width / ASPECT > canvas.height * ASPECT) {
        sw = canvas.width;
        sh = Math.floor(canvas.height * ASPECT);
    } else {
        sw = Math.floor(canvas.width / ASPECT);
        sh = canvas.height;
    }

    // Copy the contents of the canvas to a WIDTH * HEIGHT thumbnail.
    let thumbnail = document.createElement("canvas");
    thumbnail.width = WIDTH;
    thumbnail.height = HEIGHT;
    let ctx = thumbnail.getContext("2d");
    ctx.fillStyle = "rgb(248, 249, 250)" // bg-light
    ctx.fillRect(0, 0, WIDTH, HEIGHT);
    ctx.drawImage(
        canvas, 0, 0, sw, sh, 0, 0, WIDTH, HEIGHT
    );

    // Upload the thumbnail to the server.
    thumbnail.toBlob(blob => {
        let data = new FormData();
        data.append("image", blob, "thumbnail.png");
        data.append("thumbnail", scene_uuid);

        fetch("/api/upload", { method: "POST", body: data });
    });
}

// End :: Externs

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

function active_scene() {
    return RustFuncs.active_scene();
}
