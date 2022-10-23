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

refresh_projects();
