# Frontend

The frontend for this app is built using Bootstrap. It uses pre-rendered
single-file HTML pages, with all JS, CSS, etc included. This is accomplished
with the python script `build.py` which acts as a preprocessor for files in
`./pages/`, performing simple string substitutions to build the pages for
serving. This allows the initial page load to occur in a single request.

# Syntax

The preprocessor attempts substitution on double brace pairs (`{{ }}`). Several
possible substitution styles are possible.

* File substitution: `{{ file.ext }}` will read `file.ext` from `./include/` and 
    insert the full text of the file in place of this token.
* Function calls: `{{ funcname(argument) }}` will attempt to replace the token
    with the result of calling `funcname` with `argument`. The current functions
    are:
    * `javascript(https://myurl/main.js)` will download `main.js` (caching under
        `./include/.cache/`) and replace the token with the full text of this
        file, enclosed in a `<script>` tag.
    * `stylesheet(https://myurl/styles.css)` will download `styles.css` (again,
        cached) and replace, enclosed in a `<style>` tag.
    * `bootstrap_icon(icon-name)` will download the specified icon SVG (cached)
        and replace.
