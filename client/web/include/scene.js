const Icons = {
    check_circle: `{{ bootstrap_icon(check-circle) }}`,
    exclamation_triangle: `{{ bootstrap_icon(exclamation-triangle) }}`,
    plus_lg: `{{ bootstrap_icon(plus-lg) }}`
};

window.onload = () => {
    view_media();  
};

function preview_card(src, name, status_indicator = true) {
    let parent = document.createElement("div");
    parent.classList.add("col", "pt-2");
    let card = document.createElement("div");
    card.classList.add("card");
    parent.appendChild(card);
    let image = document.createElement("img");
    image.classList.add("card-img-top");
    image.style.height = "6em";
    image.style.objectFit = "cover";
    image.src = src;
    card.appendChild(image);
    let body = document.createElement("div");
    body.classList.add("card-body");
    card.appendChild(body);
    let title = document.createElement("p");
    title.classList.add("card-text", "text-truncate");
    title.innerText = name;
    body.appendChild(title);
    let error = document.createElement("p");
    error.classList.add("card-text", "text-danger");
    body.appendChild(error);
    
    if (status_indicator) {
        let spinner = document.createElement("div");
        spinner.classList.add("spinner-border", "status-indicator");
        spinner.role = "status";
        card.appendChild(spinner);    
    }

    return parent;
}

function media_card(media_item) {
    let card = preview_card(media_item.url, media_item.title, false);
    
    let image = card.querySelector(".card-img-top");
    image.style.height = "8rem";
    image.setAttribute("data-id", media_item.id);

    let add = document.createElement("button");
    add.classList.add("btn", "btn-primary");
    add.innerHTML = "Add " + Icons.plus_lg;
    add.onclick = () => add_to_scene(image);

    card.querySelector(".card-body").appendChild(add);

    return card;
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
        let card = preview_card(URL.createObjectURL(file), file.name);

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

function view_media() {
    let req = new XMLHttpRequest();
    let label = document.getElementById("media_view_error");

    let loading = document.getElementById("media_view_loading");
    loading.classList.add("show");
    
    let media_preview = document.getElementById("media_view_previews");
    media_preview.innerHTML = "";

    req.onerror = () => {
        label.classList.remove("d-none");
        label.innerText = "Network error.";
    };

    req.onload = () => {
        if (req.response.success) {
            label.classList.add("d-none");
            req.response.items.forEach(item => {
                media_preview.appendChild(media_card(item));
            });
        }
        else {
            label.classList.remove("d-none");
            label.innerText = req.response.message;
        }

        loading.classList.remove("show");
    };
    
    req.responseType = "json";
    req.open("GET", "/media");
    req.send();
}
