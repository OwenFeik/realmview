# TODO

* Layers
    * Select from selected layer first (possibly)
* Map size
    * Make map size configurable, as setting on scene, editable from project
        offcanvas
* Sprites
    * Delete sprite selection
    * Move sprite selection to other layer
* Media
    * Delete media items
    * Might be better if we never exposed media IDs, just used the URLs
    * Media library
        * Place media in directories
        * Multiselect media, apply action to all items
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
        * Users should be either a player or an editor
        * Editor should be able to configure permissions by layer, allowing or
            forbidding users to
            
            * Move sprites
            * Add sprites
    * Editors should be able to change the current scene to another in the
        project, should be able to save the project. 
* Callum requests
    * docs.google.com/document/d/1uKsAKS-huxNqc4kuHFot0McXTLlT3p83ojEalAcBtK0/
