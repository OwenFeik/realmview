function record_to_element(project) {
    let scene_list = project.scene_list.reduce((html, scene) => {
        return html + `{{ projects/scene() }}`;
    }, '');
    return template_to_element(`{{ projects/project.html }}`);
}

function update_project_title(project_uuid, project_title) {
    let body = {
        title: project_title,
    };

    fetch("/api/project/" + project_uuid, {
        method: "PATCH",
        body: JSON.stringify(body),
        headers: { "Content-Type": "application/json" }
    });
}

function delete_project(project_uuid, project_title) {
    modal_confirm(
        () => {
            fetch("/api/project/" + project_uuid, { method: "DELETE" }).then(
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
