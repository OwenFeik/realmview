# Frontend

The frontend for this app is built using Bootstrap. It uses pre-rendered
single-file HTML pages, with all JS, CSS, etc included. This is accomplished
with the python script `build.py` which acts as a preprocessor for files in
`./pages/`, performing simple string substitutions to build the pages for
serving. This allows the initial page load to occur in a single request.

# Syntax


## Braces

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

## Special

Special templates: `{{ dir/file(key=value) }}` will load
`.include/dir/file.ext` (extensions ignored) action run the template with the
key-value arguments supplied in the argument list. These templates have the
following special substitutions:

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

## HTML

There is an alternate syntax for special templates. Rather than include a file
like `{{ dir/file(key=value) }}` one can use the equivalent expression
`<DirFile key="value">`. This has a few quirks

* Tags must start with a capital letter to indicate to the preprocessor that
    they aren't normal HTML tags. The tag will be converted to lowercase and
    the file is expected to have a lowercase name. Instead of using a slash to
    indicate a directory, use a capital letter. So `dir/file` becomes
    `DirFile`.
* If a file isn't found the preprocessor will look for the concatenation of the
    tag with `Start`. For example if `Page` doesn't correspond to an
    includeable file, it will instead find `PageStart` i.e. `page/start`.
* For closing tags, like `/Page`, the preprocessor will attempt to include the
    concatenation of the tag with `End` e.g. `PageEnd` i.e. `page/end`
* Together this provides a somewhat elegant way to implement tag pairs.
    `<Page a="b" c="d">Hello World</Page>` is equivalent to
    
    ```
    {{ page/start(a="b", c="d") }}
        Hello World
    {{ page/end() }}` 
    ```
