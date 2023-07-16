function record_to_element(project) {
    let scene_list = project.scene_list.reduce((html, scene) => {
        return html + `{{ projects/scene() }}`;
    }, '');
    return template_to_element(`{{ projects/project.html }}`);
}

function update_titles(project_key, project_title, scene_key, scene_title) {
    let body = {
        project_key,
        project_title,
        scene_key,
        scene_title
    };

    fetch("/api/scene/details", {
        method: "POST",
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" }
    });
}

function delete_project(project_key, project_title) {
    modal_confirm(
        () => {
            fetch("/api/project/" + project_key, { method: "DELETE" }).then(
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
            fetch("/api/scene/" + scene_key, { method: "DELETE" }).then(
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
