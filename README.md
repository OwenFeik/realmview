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
* Expose closure macro
* Added tokens can't be edited by players
* Token health bars
* Better database abstraction
* Autosave for editor as well as game.
* Scene title in save dialog for new scene should be preselected.
* Save scene when creating; assign ID immediately.
* Should avoid creating duplicate layers when players are joining.
* Resize maintaining aspect ratio.
* Queue messages when websocket isn't open.
* Loading screen when websocket disconnected for some duration.

## Bugs
* Updating sprite shape via menu is broken
* Fog of war sometimes doesn't show up
* Remove sprites is forbidden; should be able to remove own sprites
* Crashes when resizing grid
* Hollow shapes
    * When rendered with opacity have overlapping triangles visible.
    * Have wrong dimensions after resizing window until zoom.
    * Have varying stroke, obvious at higher stroke levels.
* Resizing a drawing increases the line stroke (maybe good?).
* When changing the cap on a drawing from none, force re-render.
* Sometimes players can't see all lines.
* Resize anchors on drawings are a bit weird.
* Landing page is broken for small screens.

# Documentation

## Requirements

### Build Requirements

* Make (`apt install make`)
* Rust Nightly (`rustup default nightly`)
* `wasm-pack` (`cargo install wasm-pack`)
    * Requires `pkg-config` (`apt install pkg-config`)
    * Requires OpenSSL (`apt install libssl-dev` / `yum install openssl-devel`)
    * Note: `0.10.2`, `0.10.3` found to segfault when building. `0.9.0` known
        working. (`cargo install wasm-pack@0.9.0`)
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
