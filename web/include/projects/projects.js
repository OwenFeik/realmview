function record_to_element(project) {
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
            fetch(Api.DeleteProject(project_uuid), { method: "DELETE" }).then(
                resp => resp.json().then(body => {
                    if (body.success) {
                        document
                            .getElementById("project_" + project_uuid)
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
