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
    document.getElementById("scene_title").value
        = document.getElementById("project_title").value
        = "{{ constant(DEFAULT_TITLE) }}";
    populate_scene_select();
    load_projects(...url_project_scene());
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

    if (!key_regex.test(project_key) || !key_regex.test(scene_key)) {
        return ret;
    }

    return [project_key, scene_key];
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
        .selectedOptions[0]
        .text || "Scene";
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
    opt.setAttribute("data-id", data_id);
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
            "/project/list",
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

function populate_scene_select(list = null, scene_key = null) {
    const scene_select = document.getElementById("scene_select");

    update_loading_icon("scene_select_loading", LoadingIconStates.Idle);

    while (scene_select.options.length) {
        scene_select.remove(0);
    }

    add_default_option(scene_select);
    if (list) {
        list.forEach(scene => {
            scene_select.add(new Option(scene.title, scene.scene_key));
            if (scene.scene_key === scene_key) {
                set_active_scene(scene_key);
            }
        });    
    }

    scene_select.disabled = scene_select.options.length === 1;
    update_url_project_scene();
}

function set_active_project(project_key, scene_key = null) {
    update_url_project_scene();

    if (!project_key) {
        new_project();
        return;
    }

    populate_scene_select();
    get(
        "/project/list",
        resp => {
            populate_project_select(resp.list, project_key);
            resp.list.forEach(proj => {
                if (proj.project_key === project_key) {
                    document.getElementById(
                        "project_title"
                    ).value = proj.title;
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
        new_scene();
        return;
    }

    get(
        "/scene/load/" + scene_key,
        resp => {
            document.getElementById("scene_select").value = scene_key;
            document.getElementById("scene_title").value = resp.title;
            RustFuncs.load_scene(resp.scene);
        },
        null,
        "scene_select_loading"
    );
}

function new_project() {
    document.getElementById("project_title").value = 
        "{{ constant(DEFAULT_TITLE) }}";
    populate_scene_select();
    new_scene();
}

function new_scene() {
    document.getElementById("scene_title").value =
        "{{ constant(DEFAULT_TITLE) }}";
    let proj_id = parseInt(
        document
            .getElementById("project_select")
            .selectedOptions[0]
            .getAttribute("{{ constant(DATA_ID_ATTR) }}")
    );
    RustFuncs.new_scene(proj_id);
}

function save_project() {
    post(
        "/scene/save",
        {
            // struct SceneSaveRequest
            project_title: document.getElementById("project_title").value,
            title: document.getElementById("scene_title").value,
            encoded: RustFuncs.export_scene()
        },
        resp => {
            load_projects();
            RustFuncs.load_scene(resp.scene);
        },
        null,
        "save_project_loading"
    );
}
