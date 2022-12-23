class MediaItem {
    constructor(key, title, url, w, h) {
        this.key = key;
        this.title = title;
        this.url = url;
        this.w = w;
        this.h = h;

        this.card = template_to_element(
            `{{ media/card(IFDEF(SCENE) {{ scene=1 }}) }}`
        );

        this.image = this.card.querySelector("img");
    }

    set_attr(key, value) {
        this[key] = value;
        this.image.setAttribute("data-" + key, value);
    }

    update(obj) {
        Object.entries(obj).forEach(([k, v]) => this.set_attr(k, v));
    }

    delete() {
        fetch("/api/media/" + this.key, { method: "DELETE" });
        this.card.remove();
    }

    selected() {
        return this.card.querySelector(".form-check-input").checked;
    }
}

class MediaManager {
    constructor() {
        this.media = new Map();
    }

    add_item(resp_item) {
        let i = resp_item;
        let media_item = new MediaItem(i.media_key, i.title, i.url, i.w, i.h);
        this.media.set(resp_item.media_key, media_item);
        return media_item;
    }
    
    update_item(media_key, obj) {
        this.media.get(media_key)?.update(obj);
    }

    delete_item(media_key, confirm = true) {
        let item = this.media.get(media_key);
        if (item) {
            const deleteItem = () => {
                item.delete();
                this.media.delete(media_key);
            };

            if (confirm) {
                modal_confirm(
                    deleteItem, 
                    `Permanently delete "${item.title}"?`
                );
            } else {
                deleteItem();
            }
        }
    }

    delete_selected() {
        let to_delete = this
            .media_list()
            .filter(item => item.selected())
            .map(item => item.key);

        modal_confirm(() => {
            to_delete.map(key => this.delete_item(key, false));
        }, `Permanently delete ${to_delete.length} pieces of media?`);
    }

    load_media_with_key(media_key, callback) {
        let media = this.media.get(media_key);
        if (media) {
            if (media.image.complete) {
                callback(media.image);
            }
            else {
                media.image.addEventListener(
                    "load", () => callback(media.image)
                );
            }
        }
        else {
            get(
                "/api/media/" + media_key,
                resp => {
                    if (!resp.success) {
                        return;
                    }

                    this.add_item(resp.details);
                    this.load_media_with_key(media_key, callback);
                }
            );    
        }        
    }

    media_list() {
        return Array.from(this.media.values());
    }
}

function preview_card(src, name) {
    // uses src and name
    return template_to_element(`{{ media/preview_card.html }}`);
}

function spinner_to_icon(spinner, icon, klasse) {
    spinner.classList.remove("spinner-border");
    spinner.innerHTML = icon;
    spinner.classList.add(klasse);
    spinner.firstChild.style.display = "inline-block";
}

function set_card_error(card, message) {
    card.querySelector(".text-danger").innerText = message;
    let spinner = card.querySelector(".spinner-border");
    spinner_to_icon(spinner, Icons.exclamation_triangle, "text-danger");
}

function set_card_success(card) {
    let spinner = card.querySelector(".spinner-border");
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
        req.open("POST", "/api/upload");
        req.send(data);
    }
}

function show_media(media_list) {
    let media_preview = document.getElementById("media_view_previews");
    media_preview.innerHTML = "";
    media_list.forEach(item => {
        media_preview.appendChild(item.card);
    });
}

function view_media() {
    let req = new XMLHttpRequest();
    let label = document.getElementById("media_view_error");

    let loading = document.getElementById("media_view_loading");
    loading.classList.add("show");
    
    show_media([]);

    req.onerror = () => {
        label.classList.remove("d-none");
        label.innerText = "Network error.";
        loading.classList.remove("show");
    };

    req.onload = () => {
        if (!req.response) {
            label.classList.remove("d-none");
            label.innerText = "Network error.";
            loading.classList.remove("show");    
        }
        else if (req.response.success) {
            label.classList.add("d-none");
            show_media(
                req.response.items.map(item => media_manager.add_item(item))
            );
        }
        else {
            label.classList.remove("d-none");
            label.innerText = req.response.message;
        }

        loading.classList.remove("show");
    };
    
    req.responseType = "json";
    req.open("GET", "/api/media/list");
    req.send();
}

function search_filter(query) {
    query = query.toLowerCase();
    let matching = [];
    
    for (const item of media_manager.media.values()) {
        if (item.title.toLowerCase().includes(query)) {
            matching.push(item);
        }
    }

    show_media(matching);
}

var media_manager = new MediaManager();
window.addEventListener("load", view_media);
