# TODO

## Features
* Warn when saving over changes
* Layer opacity
* Use sprite menu to change texture
* Place media in directories
* Manage fog of war by player
* Specify which users are allowed to join the game
* Hold a button to rotate a sprite
* Save perms, reload when the same scene is loaded
* Added tokens can't be edited by players
* Token health bars
* Better database abstraction
* Queue messages when websocket isn't open.
* Loading screen when websocket disconnected for some duration.
* Aura
    * Circular, cone shape
    * Option to show only to DM or both DM and players
    * Multiple auras don't add opacity, blend by mixing colour, constant %
    * Image pattern in aura
* Preview snapping, show dimensions
* Scale battlemap by dragging 3*3 on grid.
* Add a playground for non-logged-in users.
* Add button to project page to launch game.
* When creating a new project, should go straight to scene editor.

## Bugs

* When hitting launch game there's a navigation warning.
* Disconnect while not doing anything is too fast.
* Nebula image added as solid colour. Fixed on refresh.
* Only able to select sprites created on current websocket.
* Containing rect for freehand sprites should consider stroke width.

### Low priority.

* Hollow shapes, when rendered with opacity, have overlapping triangles visible.
* A user with the same name as a default layer will be granted permission over
    that layer.
* Event sets aren't always handled correctly with permissions.

# Documentation

## Requirements

### Build Requirements

* Make (`apt install make`)
* Rust Nightly (`rustup default nightly`)
* `wasm-pack` (`cargo install wasm-pack`)
    * Requires `pkg-config` (`apt install pkg-config`)
    * Requires OpenSSL (`apt install libssl-dev` / `yum install openssl-devel`)
* Sqlite3 (`apt install sqlite3`)
* Python 3.6 or greater.
    * `urllib3` (`python3 -m pip install urllib3`)

## Api

- `/api`
    - `/auth`
        - `/login` method `POST` body `struct LoginRequest` performs login.
        - `/test` method `POST` validates session based on cookie.
        - `/logout` method `POST` terminates session if cookie present.
    - `/game`
        - `/new` method `POST` body `struct NewGameRequest` creates a new game.
        - `/{game_key}/end` method `POST` terminates a game.
        - `/{game_key}` method `POST` tests if a given game exists.
        - `/{game_key}` method `GET` joins a game, upgrading to websocket.
    - `/project`
        - `/save` method `POST` body `struct Save` (binary), creates or updates
            a project.
        - `/list` method `GET` returns list of projects for authenticated user.
        - `/new` method `POST` body `struct NewProjectRequest` creates a new
            project.
        - `/{uuid}` method `GET` returns information about this project.
        - `/{uuid}/save` method `GET` returns `struct ProjectDataResponse` for
            the requested project.
        - `/{uuid}` method `POST` body `struct ProjectDetailsRequest` updates
            project title.
        - `/{uuid}` method `DELETE` deletes the specified project.
    - `/media`
        - `/list` method `GET` returns list of media for authenticated user.
        - `/details` method `POST` body `struct DetailsUpdate` updates metadata
            for a media item.
        - `/{uuid}` method `GET` returns information about a media item.
        - `/{uuid}` method `DELETE` deletes a media item.
    - `/register` method `POST` body `struct RegistrationRequest` registers a
        new user.

## Pages

- `/project/{uuid}` is the project editor page for the given project.

## Moving Sprites

* Use the select tool to drag sprites around. Click on a sprite and drag to
    move or draw a marquee to select multiple sprites.
* When dragging a sprite, if that sprite was initially aligned to the grid, it
    will be snapped to the grid on finishing the move. If it was initially
    unaligned, it will remain so. To change this behaviour, hold `Alt`. So If
    you want to unalign a sprite from the grid, hold `Alt` when releasing a
    drag. Likewise to snap an unaligned sprite to the grid. 

## Permissions

Permissions are segmented by layer and by sprite. A user may have permissions
over a given sprite, in which case they can modify and delete that sprite, or a
layer, in which case they can edit or delete any sprite in that layer. A user
may have permissions over any number of sprites or layers. Permission on a layer
does not grant permission to edit the visibility or lock state of the layer, or
the ability to delete the layer.

There are four roles a user in a game may have, which each confer different
default permissions.

* Spectators may only view the scene. They cannot interact with any entity.
* Players are granted a single layer to edit on joining the game. They may be
    granted permissions over additional layers or sprites by an editor or
    better.
* Editors have full permission over all aspects of the scene, including to edit
    layer visibility and locks state, and delete layers. They may edit the
    permissions of players or spectators. They may edit the fog of war.
* The owner of the scene has a special role which grants irrevocable editor
    permissions and the ability to grant the editor role.
