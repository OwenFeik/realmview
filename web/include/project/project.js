function record_to_element(project) {
    let scene_list = project.scene_list.reduce((html, scene) => {
        return html + `{{ project/scene.html }}`;
    }, '');
    return template_to_element(`{{ project/project.html }}`);
}

function refresh_projects() {
    fetch("/project/list").then(resp => resp.json().then(data => {
        if (!data.success) {
            return;
        }

        let list = document.getElementById("project_list")
        data.list.forEach(
            project => list.appendChild(record_to_element(project))
        );
    }))
}

refresh_projects();
