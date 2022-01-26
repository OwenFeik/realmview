function form_to_json(form) {
    let data = {};
    let i = 0;
    while (true) {
        let element = form.elements[i++];
        if (element === undefined) {
            break;
        }
        
        if (element.tagName === "INPUT") {
            data[element.id] = element.value;
        }
    }

    return JSON.stringify(data);
}

function post_form_json(form, route) {
    let req = new XMLHttpRequest();

    req.onerror = () => {
        form.querySelector("#error_message")
            .innerText = "Network error. Please try again later.";
    }

    req.onload = () => {
        if (req.response.success) {
            alert("Registered!");
        }
        else if (req.response.problem_field) {
            let input = form.querySelector("#" + req.response.problem_field);
            input.setCustomValidity(req.response.message);
            input.oninput = () => input.setCustomValidity("");
        }
        else {
            form.querySelector("#error_message")
                .innerText = req.response.message;
        }
    }

    req.responseType = "json";
    req.open("POST", route);
    req.setRequestHeader("Content-Type", "application/json;charset=UTF-8");
    req.send(form_to_json(form));
}

function submit_form(form, route) {
    if (form.classList.contains("needs-validation")) {
        form.classList.add("was-validated");
        if (!form.checkValidity()) {
            return;
        }
    }

    post_form_json(form, route);
}
