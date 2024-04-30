# TODO

## Features
* Warn when saving over changes
* Layer opacity
* Use sprite menu to change texture
* Place media in directories
* Manage fog of war by player
* Specify which users are allowed to join the game
* Measurements, persistent measurements
* Hold a button to rotate a sprite
* Save perms, reload when the same scene is loaded
* Simplify perms.
* Added tokens can't be edited by players
* Token health bars
* Better database abstraction
* Should avoid creating duplicate layers when players are joining.
* Queue messages when websocket isn't open.
* Loading screen when websocket disconnected for some duration.
* Aura
    * Circular, cone shape
    * Option to show only to DM or both DM and players
    * Multiple auras don't add opacity, blend by mixing colour, constant %
    * Image pattern in aura
* Preview snapping, show dimensions
* Scale battlemap by dragging 3*3 on grid.
* Better positioning for line / cone labels (just put near head?)
* Better bounding boxes for lines / cones. Probably just update rect, remove
    excess points when done?
* Allow manual saving in game.
* Add a playground for non-logged-in users.

## Bugs

* If someone else resizes a drawing the drawing isn't re-rendered for others.
* Editor can't remove lines added by player during game.
* Drawings sometimes vibrate a bit as they are drawn.
* Drawing labels are positioned incorrectly.

### Low priority

* Hollow shapes
    * When rendered with opacity have overlapping triangles visible.
    * Have wrong dimensions after resizing window until zoom.
    * Have varying stroke, obvious at higher stroke levels.
* Resizing a drawing increases the line stroke (maybe good?).

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
