function submit_project_form() {
    const FORM_ID = "project_form";
    post_form_json(FORM_ID, resp => {
        // resp is a struct NewProjectResponse
        if (resp.success) {
            let title = 'Project - ' + resp.title; 
            history.pushState(title, "", "/project/" + resp.project_key);
            document.title = title;
        }
    })
}
