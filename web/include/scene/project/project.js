window.addEventListener("load", () => {
    const project_select = document.getElementById("project_select");
    const scene_select = document.getElementById("scene_select");

    get(
        "/project/list",
        resp => populate_project_select(resp.list)
    );

    add_default_option(project_select);
    project_select.onchange = e => {
        let project_key = e.target.value;
        if (project_key) {
            set_active_project(project_key);
        }
    };

    add_default_option(scene_select);
    scene_select.onchange = e => {
        let scene_key = e.target.value;
        if (scene_key) {
            set_active_scene(scene_key);
        }
    };
});

function add_default_option(select) {
    select.add(new Option("None", ""));
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
        }
    });
}

function populate_scene_select(list) {
    const scene_select = document.getElementById("scene_select");

    while (scene_select.options.length) {
        scene_select.remove(0);
    }

    add_default_option(scene_select);
    list.forEach(scene => {
        scene_select.add(new Option(scene.title, scene.scene_key));
    });
}

function set_active_project(project_key) {
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
        }
    );
}

function set_active_scene(scene_key) {
    get(
        "/scene/load/" + scene_key,
        resp => {
            document.getElementById("scene_title").value = resp.title;
            rust_funcs.load_scene(resp.scene);
        }
    );
}

function save_project() {
    post(
        "/scene/save",
        {
            title: document.getElementById("scene_title").value,
            encoded: rust_funcs.export_scene()
        },
        resp => console.log(rust_funcs.load_scene(resp.scene))
    );
}
