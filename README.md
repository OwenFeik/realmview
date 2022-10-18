# TODO

* Scene
* Layers
    * Select from selected layer first (possibly)
    * Layer opacity
* Sprites
    * When dragging a sprite, if it isn't locked to the grid at the start, it
        shouldn't snap at the end
    * Use sprite menu to change texture
    * Group sprites
* Media
    * Media library
        * Place media in directories
        * Multiselect media, apply action to all items
    * Pressing enter in modal breaks things
* Projects
    * /project/PROJECT_KEY should show a page with the scenes in that project
        and project metadata
    * /project should show the same page but empty, with prompt to save the
        project and create a scene
* Game
    * Save the scene in the game
    * Associate each client with a user. When a user connects a second time,
        terminate that user's other sessions
    * Specify which users are allowed to join the game
    * Feedback for when connecting to a game as a given CLIENT_KEY fails
    * Permission levels
        * Editor should be able to configure permissions by layer, allowing or
            forbidding users to
            
            * Move sprites
            * Add sprites
    * Editors should be able to change the current scene to another in the
        project, should be able to save the project. 
* Callum requests
    * docs.google.com/document/d/1uKsAKS-huxNqc4kuHFot0McXTLlT3p83ojEalAcBtK0/

## Permissions

* Owner / Editor
    * All tools enabled without restriction
    * ~~All menus available~~
    * ~~Meta controls available~~
* Player
    * All tools enabled
        * Select tool only works on sprites which user has update permission on
        * Can only delete sprites that user has remove permission for
    * ~~Layer and scene menus hidden~~
    * ~~Meta controls hidden~~
* Spectator
    * All tools disabled except pan
    * ~~All menus hidden~~
    * ~~Meta controls hidden~~
