window.addEventListener("load", () => {
    const project_select = document.getElementById("project_select");
    const scene_select = document.getElementById("scene_select");

    add_default_option(project_select);
    project_select.onchange = e => {
        let project_key = e.target.value;
        if (project_key) {
            set_active_project(project_key);
        }
    };

    scene_select.disabled = true;
    scene_select.onchange = e => {
        let scene_key = e.target.value;
        if (scene_key) {
            set_active_scene(scene_key);
        }
    };

    load_projects();
});

function add_default_option(select) {
    select.add(new Option("-", ""));
}

function load_projects() {
    const project_select = document.getElementById("project_select");

    let current = project_select.value; 
    get(
        "/project/list",
        resp => populate_project_select(resp.list, current)
    );
}

function populate_project_select(list, project_key = null) {
    const project_select = document.getElementById("project_select");

    while (project_select.options.length) {
        project_select.remove(0);
    }

    add_default_option(project_select);
    list.forEach(proj => {
        project_select.add(new Option(proj.title, proj.project_key));
        if (proj.project_key === project_key) {
            project_select.value = project_key;
            populate_scene_select(
                proj.scene_list,
                document.getElementById("scene_select").value
            );
        }
    });
}

function populate_scene_select(list = null, scene_key = null) {
    const scene_select = document.getElementById("scene_select");

    while (scene_select.options.length) {
        scene_select.remove(0);
    }

    add_default_option(scene_select);
    if (list) {
        list.forEach(scene => {
            scene_select.add(new Option(scene.title, scene.scene_key));
            if (scene.scene_key === scene_key) {
                scene_select.value = scene_key;
            }
        });    
    }

    scene_select.disabled = scene_select.options.length === 1;
}

function set_active_project(project_key) {
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
                }
            });
        },
        null,
        "project_select_loading"
    );
}

function set_active_scene(scene_key) {
    get(
        "/scene/load/" + scene_key,
        resp => {
            document.getElementById("scene_title").value = resp.title;
            rust_funcs.load_scene(resp.scene);
        },
        null,
        "scene_select_loading"
    );
}

function save_project() {
    post(
        "/scene/save",
        {
            // struct SceneSaveRequest
            project_title: document.getElementById("project_title").value,
            title: document.getElementById("scene_title").value,
            encoded: rust_funcs.export_scene()
        },
        resp => {
            load_projects();
            rust_funcs.load_scene(resp.scene);
        },
        null,
        "save_project_loading"
    );
}
