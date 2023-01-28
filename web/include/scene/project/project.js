window.addEventListener("load", () => {
    const project_select = document.getElementById("project_select");
    const scene_select = document.getElementById("scene_select");

    add_default_option(project_select);
    project_select.oninput = e => set_active_project(e.target.value);

    scene_select.disabled = true;
    scene_select.oninput = e => set_active_scene(e.target.value);

    configure_loading_icon_reset();
    load_project_scene();
});

function load_project_scene() {
    set_title("project");
    set_title("scene");
    const [proj_key, scene_key] = url_project_scene();
    populate_scene_select();
    load_projects(proj_key, scene_key);
}

// Set up loading icons so that they reset to the idle state when the offcanvas
// is closed.
function configure_loading_icon_reset() {
    const offcanvas = document.getElementById("project_offcanvas");
    offcanvas.addEventListener(
        "hidden.bs.offcanvas",
        () => {
            offcanvas.querySelectorAll(".loading-icon").forEach(
                icon => update_loading_icon(icon.id, LoadingIconStates.Idle)
            );
        }
    );
}

function url_project_scene() {
    const parts = url_parts();

    const ret = [null, null];

    if (parts.length != 4) {
        return ret;
    }

    // Parts should be:
    // ["project", "PROJECT_KEY", "scene", "SCENE_KEY"]

    if (parts[0] != "project" || parts[2] != "scene") {
        return ret;
    }

    const key_regex = /^[0-9A-F]{{{ constant(SCENE_KEY_LENGTH) }}}$/;

    let project_key = parts[1];
    let scene_key = parts[3];

    if (key_regex.test(project_key)) {
        ret[0] = project_key;
    }

    if (key_regex.test(scene_key)) {
        ret[1] = scene_key;
    }

    return ret;
}

function is_scene_editor() {
    let page = url_parts()[0];
    return page == "project" || page == "scene" || page == "scene.html";
}

function update_url_project_scene() {
    if (!is_scene_editor()) {
        return;
    }

    let scene_title = document
        .getElementById("scene_select")
        ?.selectedOptions[0]
        ?.text || "Scene";
    document.title = scene_title;
    page_title = "Scene - " + scene_title;

    const project_key = selected_project();
    const scene_key = selected_scene();

    if (!selected_project()) {
        history.pushState(page_title, "", "/project/new/scene/new");
    }
    else if (!scene_key) {
        history.pushState(
            page_title, "", `/project/${project_key}/scene/new`
        );
    }
    else {
        history.pushState(
            page_title, "", `/project/${project_key}/scene/${scene_key}`
        );
    }
}

function create_option(label, value, data_id) {
    let opt = new Option(label, value);

    if (data_id) {
        opt.setAttribute("data-id", data_id);
    }
    return opt;
}

function add_default_option(select, label = "New") {
    select.add(create_option(label, "", 0));
}

function selected_project() {
    return document.getElementById("project_select").value;
}

function selected_scene() {
    return document.getElementById("scene_select").value;
}

function load_projects(project_key = null, scene_key = null) {
    if (project_key) {
        set_active_project(project_key, scene_key);
    }
    else {
        get(
            "/api/project/list",
            resp => populate_project_select(
                resp.list,
                project_key || selected_project(),
                scene_key
            )
        );
    }
}

function populate_project_select(list, project_key = null, scene_key = null) {
    const project_select = document.getElementById("project_select");

    update_loading_icon("project_select_loading", LoadingIconStates.Idle);

    while (project_select.options.length) {
        project_select.remove(0);
    }

    add_default_option(project_select);
    list.forEach(proj => {
        project_select.add(
            create_option(proj.title, proj.project_key, proj.id)
        );

        if (proj.project_key === project_key) {
            project_select.value = project_key;
            populate_scene_select(
                proj.scene_list,
                scene_key || selected_scene()
            );
        }
    });

    project_select.disabled = project_select.options.length === 1;
    update_url_project_scene();
}

function add_scene_entries(select, list, default_option = true) {
    if (!select) {
        return;
    }

    while (select.options.length) {
        select.remove(0);
    }

    if (default_option) {
        add_default_option(select);
    }

    list.forEach(scene => {
        select.add(new Option(scene.title, scene.scene_key));
    });

    select.disabled = select.options.length <= 1;
}

function populate_scene_select(list = null, scene_key = null) {
    const scene_select = document.getElementById("scene_select");

    update_loading_icon("scene_select_loading", LoadingIconStates.Idle);

    if (list) {
        add_scene_entries(scene_select, list);
        list.forEach(scene => {
            if (scene.scene_key === scene_key) {
                set_active_scene(scene_key);
            }
        });    
    }

    update_url_project_scene();
}

function set_title(name, title) {
    let input = document.getElementById(name + "_title");
    let button = input.parentNode.querySelector("button");
    if (title) {
        input.value = title;
        input.disabled = true;
        button.innerHTML = Icons.pencil_square;
    } else {
        input.value = "{{ constant(DEFAULT_TITLE) }}";
        input.disabled = false;
        button.innerHTML = Icons.check_circle;
    }
}

function set_active_project(project_key, scene_key = null) {
    update_url_project_scene();

    if (!project_key) {
        create_new_project();
        return;
    }

    populate_scene_select();
    get(
        "/api/project/list",
        resp => {
            populate_project_select(resp.list, project_key);
            resp.list.forEach(proj => {
                if (proj.project_key === project_key) {
                    set_title("project", proj.title);
                    populate_scene_select(proj.scene_list);
                    set_active_scene(scene_key);
                }
            });
        },
        null,
        "project_select_loading"
    );
}

function set_active_scene(scene_key) {
    update_url_project_scene();

    if (!scene_key) {
        create_new_scene();
        return;
    }

    get(
        "/api/scene/load/" + scene_key,
        resp => {
            document.getElementById("scene_select").value = scene_key;
            set_title("scene", resp.title);

            if (resp.success) {
                load_scene(resp.scene);
            }
            update_url_project_scene();
        },
        null,
        "scene_select_loading"
    );
}

function create_new_project() {
    set_title("project");
    populate_scene_select();
    create_new_scene();
}

function create_new_scene() {
    set_title("scene", document.getElementById("scene_title").value);

    if (!selected_scene()) {
        // Already a new scene, don't overwrite.
        return;
    }

    let proj_id = parseInt(
        document
            .getElementById("project_select")
            .selectedOptions[0]
            .getAttribute("{{ constant(DATA_ID_ATTR) }}")
    );
    new_scene(proj_id);
}

function update_select(id, label, value, data_id = null) {
    const select = document.getElementById(id);
    const option = create_option(label, value, data_id);

    if (current = select.querySelector(`option[value='${value}']`)) {
        select.replaceChild(option, current);
    }
    else {
        select.appendChild(option);
    }
    select.value = value;
}

function upload_thumbnail() {
    const ASPECT = 4 / 3;
    const WIDTH = 256;
    const HEIGHT = WIDTH / ASPECT;

    // If no scene is selected, we don't know which scene the thumbnail is for.
    let scene = selected_scene();
    if (!scene) {
        return;
    }

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
        data.append("thumbnail", scene);

        fetch("/api/upload", { method: "POST", body: data });
    });
}

function save_project() {
    let proj = selected_project();
    post(
        "/api/scene/save",
        {
            // struct SceneSaveRequest
            project_title: document.getElementById("project_title").value,
            title: document.getElementById("scene_title").value,
            encoded: RustFuncs.export_scene()
        },
        resp => {
            if (resp.success) {
                load_scene(resp.scene);
            }

            // Only update if the selected project is unchanged
            if (selected_project() === proj) {
                update_select("scene_select", resp.title, resp.scene_key);
                update_select(
                    "scene_menu_change_scene", resp.title, resp.scene_key
                );
                update_select(
                    "project_select",
                    resp.project_title,
                    resp.project_key,
                    resp.project_id
                );
                update_url_project_scene();
            }

            upload_thumbnail();
        },
        null,
        "save_project_loading"
    );
}
