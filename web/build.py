import json
import os
import re
import sys
import typing
import urllib3  # type: ignore


SCRIPT_DIR = os.path.abspath(os.path.dirname(__file__))
INCLUDE_DIR = os.path.join(SCRIPT_DIR, "include")
INCLUDE_CACHE_DIR = os.path.join(INCLUDE_DIR, ".cache")
PAGES_DIR = os.path.join(SCRIPT_DIR, "pages")


def output_directory() -> str:
    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = os.path.join(SCRIPT_DIR, "output")

    return output_dir


def substitution_regex() -> re.Pattern:
    # Identifier characters
    ic = r"[a-zA-Z0-9_]"

    # "full characters" or something. Anything that could appear in a used
    # URL, function argument, file name, etc
    fc = r"[a-zA-Z0-9_.:@/\-]"

    # function name followed by a single argument
    function_regex = rf"(?P<func>{ic}+)\((?P<arg>{fc}*)\)"

    # file path (relative to include/special) followed by comma separated
    # k = v args with any amount of whitespace between them
    kwarg_file_regex = (
        rf"(?P<kwarg_file>{fc}+)"
        rf"\(\s*(?P<args>({ic}+\s*=\s*({fc}+|\"[^\"]+\")(,\s*|\s*(?=\))))*)\)"
    )

    # file name
    include_regex = rf"(?P<file>{fc}+)"

    substitution_types = [function_regex, kwarg_file_regex, include_regex]

    overall_regex = r"{{\s*(" + r"|".join(substitution_types) + r")\s*}}"

    return re.compile(overall_regex)


def ensure_cache_dir() -> str:
    if not os.path.isdir(INCLUDE_CACHE_DIR):
        os.makedirs(INCLUDE_CACHE_DIR)

    gitignore_path = os.path.join(INCLUDE_CACHE_DIR, ".gitignore")
    if not os.path.isfile(gitignore_path):
        with open(gitignore_path, "w") as f:
            f.write("*")

    return INCLUDE_CACHE_DIR


def load_cached_file(filename: str) -> typing.Optional[str]:
    cached_file = os.path.join(ensure_cache_dir(), filename)
    if os.path.isfile(cached_file):
        try:
            with open(cached_file, "r") as f:
                return f.read()
        except:
            # File is corrupted or something, we'll overwrite it with a new one.
            pass
    return None


def cache_file(filename: str, content: str) -> None:
    with open(os.path.join(ensure_cache_dir(), filename), "w") as f:
        f.write(content)


def download_resource(url: str) -> str:
    resp = urllib3.PoolManager().request("GET", url)
    if resp.status != 200:
        print(f"Failed to retrieve {url} status {resp.status}.")
        exit(os.EX_DATAERR)
    return resp.data.decode("utf-8")


def bootstrap_icon(name: str) -> str:
    URL_FORMAT = "https://icons.getbootstrap.com/assets/icons/{}"

    filename = f"{name}.svg"
    if cached_file := load_cached_file(filename):
        return cached_file
    svg = download_resource(URL_FORMAT.format(filename))
    cache_file(filename, svg)
    return svg


def filename_from_url(url: str) -> str:
    parts = url.split("/")

    try:
        i = 1
        while not parts[-i]:
            i += 1
        return parts[-i]
    except IndexError:
        print(f"Couldn't determine filename for {url}")
        exit(os.EX_SOFTWARE)


def load_url(url: str) -> str:
    filename = filename_from_url(url)
    if content := load_cached_file(filename):
        return content
    content = download_resource(url)
    cache_file(filename, content)
    return content


def load_resource(resource: str) -> str:
    if resource.startswith("http://") or resource.startswith("https://"):
        return load_url(resource)
    else:
        return include_file(resource)


def stylesheet(resource: str) -> str:
    return f"<style>{load_resource(resource)}</style>"


def javascript(resource: str) -> str:
    return f"<script>{load_resource(resource)}</script>"


def constant(
    name: str, constants: typing.Dict[str, typing.Union[str, int]] = {}
) -> str:
    if constants:
        try:
            return str(constants[name])
        except KeyError:
            print(f"Missing constant: {name}. Aborting.")
            exit(os.EX_NOINPUT)
    else:
        with open(os.path.join(SCRIPT_DIR, "constants.json"), "r") as f:
            constants.update(json.load(f))
        return constant(name)


def function_from_name(
    funcs: typing.List[typing.Callable], name: str
) -> typing.Callable:
    try:
        return {f.__name__: f for f in funcs}[name]
    except KeyError:
        print(f"Missing function: {name}. Aborting.")
        exit(os.EX_NOINPUT)


def function_substitution(func: str, arg: str) -> str:
    functions = [bootstrap_icon, stylesheet, javascript, constant]
    args = [s.strip() for s in arg.split(",")]
    return function_from_name(functions, func)(*args)  # type: ignore


def read_block(identifier: str, html: str) -> str:
    if not (match := re.search(re.escape(identifier) + r"\s*{{", html)):
        raise ValueError(f"Missing indentifier: {identifier}")

    n_braces = 2
    i = match.end() + 1
    while i < len(html) and n_braces:
        if html[i] == "{":
            n_braces += 1
        elif html[i] == "}":
            n_braces -= 1
        i += 1

    if n_braces:
        raise ValueError("Unterminated block.")

    return html[match.start() : i]


def block_contents(block: str) -> str:
    return re.sub(r"^.*?{{", "", block)[:-2]


# Look away. This parses a file to check for a preprocessor block preceded by
# the identifier PREAMBLE at the start of the file and if it finds one, it
# reads in the python code contained in the block and executes it, allowing
# it to mutate the kwargs dict.
def process_preamble(html: str, kwargs: typing.Dict[str, str]) -> str:
    try:
        block = read_block("PREAMBLE", html)
    except ValueError:
        return html
    preamble = block_contents(block)
    exec(preamble)
    return html.replace(block, "").strip()


def process_ifdefs(html: str, kwargs: typing.Dict[str, str]) -> str:
    rx = re.compile(r"(?P<ident>(?P<cond>IFN?DEF)\((?P<arg>[A-Z_]+)\))\s*{{")
    queue: typing.List[typing.Tuple[str, str, str]] = []
    for match in re.finditer(rx, html):
        cond = match.group("cond")
        kwarg = match.group("arg")
        block = read_block(match.group("ident"), html)
        queue.append((cond, kwarg, block))

    for cond, kwarg, block in queue:
        repl = ""
        if (cond == "IFDEF" and kwarg in kwargs) or (
            cond == "IFNDEF" and kwarg not in kwargs
        ):
            repl = block_contents(block)
        html = html.replace(block, repl)

    return html


def remove_quotes(string: str) -> str:
    if string.startswith('"'):
        return string[1:-1]
    return string


# TODO doesn't handle quoted strings with commas
def kwarg_file_subsitution(file: str, args: str) -> str:
    kwargs = {
        k.upper(): remove_quotes(v)
        for k, v in map(
            lambda arg: re.split(r"\s*=\s*", arg, 1),
            [term for term in re.split(r",\s*", args.strip()) if term],
        )
    }

    file_path = os.path.join("special", file)
    html = include_file(file_path, False)

    try:
        html = process_preamble(html, kwargs)
        html = process_ifdefs(html, kwargs)
    except ValueError as e:
        print(
            f"Substitution failed for {file_path}.\nReason: {e}\nArgs: {kwargs}"
        )
        exit(os.EX_DATAERR)

    for kwarg in re.finditer(r"{{\s*(?P<k>[A-Z_]+)\s*}}", html):
        html = html.replace(kwarg.group(0), kwargs.get(kwarg.group("k"), ""))

    return process_html(html)


def include_file(file: str, process: bool = True) -> str:
    try:
        include = os.path.join(INCLUDE_DIR, file)
        with open(include, "r") as f:
            html = f.read()
    except FileNotFoundError:
        try:
            # Allow omission of file extension
            include = os.path.join(INCLUDE_DIR, file + ".html")
            with open(include, "r") as f:
                html = f.read()
        except FileNotFoundError:
            print(f"Missing include file: {file}. Aborting.")
            exit(os.EX_DATAERR)

    # Note: This could recurse until OOM if a file is self-referential.
    # Don't do that.
    if process:
        return process_html(html)
    else:
        return html


def handle_match(match: re.Match) -> str:
    if func := match.group("func"):
        return function_substitution(func, match.group("arg"))
    elif kw_file := match.group("kwarg_file"):
        return kwarg_file_subsitution(kw_file, match.group("args"))
    elif file := match.group("file"):
        return include_file(file)
    else:
        # This path is unreachable unless additional options are added to the
        # regular expression.
        return ""


def process_html(html: str) -> str:
    regex = substitution_regex()
    for match in re.finditer(regex, html):
        html = html.replace(match.group(0), handle_match(match))
    return html


def main() -> None:
    output_dir = output_directory()

    for page in os.listdir(PAGES_DIR):
        with open(os.path.join(PAGES_DIR, page), "r") as f:
            html = f.read()

        html = process_html(html)
        if "{{" in html:
            print(f"Substitution may have failed for {page}.")

        with open(os.path.join(output_dir, page), "w") as f:
            f.write(html)


if __name__ == "__main__":
    main()
