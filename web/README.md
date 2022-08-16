# Frontend

The frontend for this app is built using Bootstrap. It uses pre-rendered
single-file HTML pages, with all JS, CSS, etc included. This is accomplished
with the python script `build.py` which acts as a preprocessor for files in
`./pages/`, performing simple string substitutions to build the pages for
serving. This allows the initial page load to occur in a single request.

# Syntax

The preprocessor attempts substitution on double brace pairs (`{{ }}`). Several
possible substitution styles are possible.

* File substitution: `{{ file.ext }}` will read `file.ext` from `./include/`
    and insert the full text of the file in place of this token.
* Function calls: `{{ funcname(argument) }}` will attempt to replace the token
    with the result of calling `funcname` with `argument`. The current
    functions are:
    * `javascript(https://myurl/main.js)` will download `main.js` (caching
        under `./include/.cache/`) and replace the token with the full text of
        this file, enclosed in a `<script>` tag. Can also load local files.
    * `stylesheet(https://myurl/styles.css)` will download `styles.css` (again,
        cached) and replace, enclosed in a `<style>` tag.
    * `bootstrap_icon(icon-name)` will download the specified icon SVG (cached)
        and replace.
    * `constant(CONSTANT_NAME)` will load the specified constant value from
        `./constants.json` and substitute in the value.
    * `unique_string()` returns an 8-character UUID, guaranteed to start with a
    letter.
* Special templates: `{{ dir/file(key=value) }}` will load
    `./special/dir/file.ext` (extensions ignored) action run the template with
    the key-value arguments supplied in the argument list. These templates have
    the following special substitutions:
    * `{{ KEY }}` will be replaced with the value of `KEY` in the `kwargs`.
        Note that `key=value` becomes `{ "KEY": "value" }`.
    * `PREAMBLE {{ kwargs["key"] = "value" }}`, a `PREAMBLE` Python block will
        be `exec`ed in the relevant function, with the `kwargs` `dict` in scope
        so that it can update keys and values with logic.
    * `IFDEF(KEY) {{ substitution }}` will replace the whole expression with
        `substitution` if `KEY` is defined in `kwargs`.
    * `IFNDEF(KEY) {{ substitution }}` will perform the same substitution as
        above if `KEY` isn't in `kwargs`.
    * Both `IFDEF` and `IFNDEF` may optionally be followed by
        `ELSE {{ substitution }}` and if they are, `substitution` will be used
        if the condition fails.
