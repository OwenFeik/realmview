window.onload = refresh_projects;

function record_to_element(project) {
    let scene_list = project.scene_list.reduce((html, scene) => {
        return html + `{{ projects/scene.html }}`;
    }, '');
    return template_to_element(`{{ projects/project.html }}`);
}

function update_prompt(main = false) {
    const prompt = document.getElementById("new_prompt");
    const text = document.getElementById("new_prompt_text");
    const button = document.getElementById("new_prompt_button");

    if (main) {
        prompt.classList.add("justify-content-center", "align-items-center");
        prompt.classList.remove("justify-content-end");
        prompt.style.height = "50vh";
        text.classList.remove("d-none");
        button.classList.remove("ms-auto");
    } else {
        prompt.classList.remove(
            "justify-content-center", "align-items-center"
        );
        prompt.classList.add("justify-content-end");
        prompt.style.height = null;
        text.classList.add("d-none");
        button.classList.add("ms-auto");
    }
}

function refresh_projects() {
    fetch("/project/list").then(resp => resp.json().then(data => {
        if (!data.success) {
            update_prompt(true);
            return;
        }

        update_prompt(data.list.length == 0);

        let list = document.getElementById("project_list")
        data.list.forEach(
            project => list.appendChild(record_to_element(project))
        );
    }))
}

function update_titles(project_key, project_title, scene_key, scene_title) {
    let body = {
        project_key,
        project_title,
        scene_key,
        scene_title
    };
    
    fetch("/scene/details", {
        method: "POST",
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" }
    });
}

function delete_project(project_key, project_title) {
    modal_confirm(
        () => {
            fetch("/project/" + project_key, { method: "DELETE" }).then(
                resp => resp.json().then(body => {
                    if (body.success) {
                        document
                            .getElementById("project_" + project_key)
                            .remove();
                    }
                })
            );
        },
        (
            `Are you sure you wish to delete your project "${project_title}"?`
            + " This action is irreversible."
        )
    );
}

function delete_scene(scene_key, scene_title) {
    modal_confirm(
        () => {
            fetch("/scene/" + scene_key, { method: "DELETE" }).then(
                resp => resp.json().then(body => {
                    if (body.success) {
                        document
                            .getElementById("scene_" + scene_key)
                            .remove();
                    }
                })
            );
        },
        (
            `Are you sure you wish to delete your scene "${scene_title}"?`
            + " This action is irreversible."
        )
    );
}
