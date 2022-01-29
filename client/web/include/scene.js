const Icons = {
    check_circle: `{{ bootstrap_icon(check-circle) }}`,
    exclamation_triangle: `{{ bootstrap_icon(exclamation-triangle) }}`
};

function preview_card(file) {
    let parent = document.createElement("div");
    parent.classList.add("col", "pt-2");
    let card = document.createElement("div");
    card.classList.add("card");
    parent.appendChild(card);
    let image = document.createElement("img");
    image.classList.add("card-img-top");
    image.style.height = "6em";
    image.style.objectFit = "cover";
    image.src = URL.createObjectURL(file);
    card.appendChild(image);
    let spinner = document.createElement("div");
    spinner.classList.add("spinner-border", "status-indicator");
    spinner.role = "status";
    card.appendChild(spinner);
    let body = document.createElement("div");
    body.classList.add("card-body");
    card.appendChild(body);
    let title = document.createElement("p");
    title.classList.add("card-text", "text-truncate");
    title.innerText = file.name;
    body.appendChild(title);
    let error = document.createElement("p");
    error.classList.add("card-text", "text-danger");
    body.appendChild(error);
    return parent;
}

function spinner_to_icon(spinner, icon, klasse) {
    spinner.classList.remove("spinner-border");
    spinner.innerHTML = icon;
    spinner.classList.add(klasse);
    spinner.firstChild.style.display = "block";
    spinner.firstChild.style.width = "1.25em";
    spinner.firstChild.style.height = "1.25em";
}

function set_card_error(card, message) {
    card.querySelector(".text-danger").innerText = message;
    let spinner = card.querySelector("div.spinner-border");
    spinner_to_icon(spinner, Icons.exclamation_triangle, "text-danger");
}

function set_card_success(card) {
    let spinner = card.querySelector("div.spinner-border");
    spinner_to_icon(spinner, Icons.check_circle, "text-success");
}

function upload_media() {
    const media_input = document.getElementById("media_upload");
    const media_preview = document.getElementById("media_upload_previews");

    media_preview.innerHTML = "";
    for (const file of media_input.files) {
        let card = preview_card(file);

        media_preview.appendChild(card);

        let data = new FormData();
        data.append("image", file);
        
        let req = new XMLHttpRequest();
    
        req.onerror = () => {
            set_card_error(card, "Network error.");
        };

        req.onload = () => {
            if (req.response.success) {
                set_card_success(card);
            }
            else {
                set_card_error(card, req.response.message);
            }
        };
    
        req.responseType = "json";
        req.open("POST", "/upload");
        req.send(data);
    }
}
