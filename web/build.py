import json
import multiprocessing
import os
import pathlib
import re
import sys
import typing
import uuid
import urllib3  # type: ignore


WEB_DIR = os.path.abspath(os.path.dirname(__file__))
INCLUDE_DIR = os.path.join(WEB_DIR, "include")
INCLUDE_CACHE_DIR = os.path.join(INCLUDE_DIR, ".cache")
PAGES_DIR = os.path.join(WEB_DIR, "pages")


# Build up the regular expression used to parse the input files.

ANY = r"[\s\S]*?"

IDENTIFIER_CHARACTERS = IC = r"[a-zA-Z0-9_]"

# Anything that could appear in a used
# URL, function argument, file name, etc
FULL_CHARACTERS = FC = r"[a-zA-Z0-9_.:@/\-]"

KWARG_ARG_REGEX = rf"({IC}+\s*=\s*" rf"({FC}+|\"[^\"]*\"|'[^']*'|\|[^\|]*\|))"

OPEN = r"{{"
CLOSE = r"}}"


# file path (relative to include/) followed by comma separated
# k = v args with any amount of whitespace between them
KWARG_FILE_REGEX = (
    rf"(?P<kwarg_file>{FC}+)"
    rf"\(\s*(?P<args>({KWARG_ARG_REGEX}(,\s*|\s*(?=\))))*)\)"
)

# different syntax for the above
# <FormField type="file"> becomes form/field(type="file")
# Note that these are differentiated from normal HTML elements by the
# leading capital letter.
HTML_FILE_REGEX = (
    rf"<\s*(?P<html_file>([A-Z]|/[A-Z])[a-zA-Z0-9_]+)\s*"
    rf"(?P<html_args>({KWARG_ARG_REGEX}(\s+|\s*(?=>)))*)\s*>"
)

# function name followed by a single argument
FUNCTION_REGEX = rf"(?P<func>{IC}+)\((?P<arg>{FC}*)\)"

# file name
INCLUDE_REGEX = rf"(?P<file>{FC}+)"

# Treate as raw text
FALLBACK_REGEX = r"(?P<raw>[^{}]+)"

SUBSTITUTION_REGEXES = [
    KWARG_FILE_REGEX,
    FUNCTION_REGEX,
    INCLUDE_REGEX,
    FALLBACK_REGEX,
]

# Final regex matches all substitution types
SUBSTITUTION_REGEX = re.compile(
    rf"{OPEN}\s*("
    + r"|".join(SUBSTITUTION_REGEXES)
    + rf")\s*{CLOSE}|{HTML_FILE_REGEX}"
)


def output_directory() -> str:
    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = os.path.join(WEB_DIR, "output")

    return output_dir


def ensure_cache_dir() -> str:
    if not os.path.isdir(INCLUDE_CACHE_DIR):
        pathlib.Path(INCLUDE_CACHE_DIR).mkdir(parents=True, exist_ok=True)

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


def is_url(poss: str) -> bool:
    return poss.startswith("http://") or poss.startswith("https://")


def load_resource(resource: str) -> str:
    if is_url(resource):
        return load_url(resource)
    else:
        return include_file(resource)


def rehost(url: str) -> str:
    data = load_resource(url)
    href = filename_from_url(url)
    with open(os.path.join(output_directory(), href), "w") as f:
        f.write(data)
    return "/" + href


def stylesheet(resource: str) -> str:
    if is_url(resource):
        href = rehost(resource)
        return f'<link rel="stylesheet" href="{href}">'
    else:
        return f"<style>{load_resource(resource)}</style>"


def javascript(resource: str) -> str:
    if is_url(resource):
        href = rehost(resource)
        return f'<script src="{href}"></script>'
    else:
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
        with open(os.path.join(WEB_DIR, "constants.json"), "r") as f:
            constants.update(json.load(f))
        return constant(name)


def unique_string() -> str:
    while (s := uuid.uuid4().hex[:8].upper())[0].isdigit():
        pass
    return s


def function_from_name(
    funcs: typing.List[typing.Callable], name: str
) -> typing.Callable:
    return {f.__name__: f for f in funcs}[name]


def function_substitution(func: str, arg: str) -> str:
    functions = [
        bootstrap_icon,
        stylesheet,
        javascript,
        constant,
        unique_string,
    ]
    args = [s.strip() for s in arg.split(",") if s]

    try:
        return function_from_name(functions, func)(*args)  # type: ignore
    except KeyError:
        try:
            return kwarg_file_subsitution(func, arg)
        except SystemExit:
            print(f"Missing function: {func}. Aborting.")
            exit(os.EX_NOINPUT)


def read_block(start: int, html: str) -> str:
    started = False
    n_braces = 0
    i = start
    while i < len(html) and (n_braces or not started):
        if html[i] == "{":
            n_braces += 1
        elif html[i] == "}":
            n_braces -= 1

        if n_braces == 2:
            started = True

        i += 1

    if n_braces:
        raise ValueError("Unterminated block.")

    return html[start:i]


def read_identifier_block(identifier: str, html: str) -> str:
    if not (match := re.search(re.escape(identifier) + rf"\s*{OPEN}", html)):
        raise ValueError(f"Missing indentifier: {identifier}")
    return read_block(match.start(), html)


def block_contents(block: str) -> str:
    return re.sub(rf"^{ANY}{OPEN}", "", block)[:-2]


# Look away. This parses a file to check for a preprocessor block preceded by
# the identifier PREAMBLE at the start of the file and if it finds one, it
# reads in the python code contained in the block and executes it, allowing
# it to mutate the kwargs dict.
def process_preamble(html: str, kwargs: typing.Dict[str, str]) -> str:
    try:
        block = read_identifier_block("PREAMBLE", html)
    except ValueError:
        return html
    preamble = block_contents(block)
    exec(preamble)
    return html.replace(block, "").strip()


def read_ifdef_block(start: int, html: str) -> typing.Tuple[str, str, str]:
    if_block = read_block(start, html)
    i = start + len(if_block)
    if re.match(rf"\s*ELSE\s*{OPEN}", html[i:]):
        else_block = read_block(i, html)
    else:
        else_block = ""

    return (if_block, else_block, if_block + else_block)


BLOCK_REGEX = rf"{OPEN}{ANY}{CLOSE}"
_IFDEF_REGEX = (
    rf"(?P<ident>(?P<cond>IFN?DEF)\((?P<arg>[A-Z_]+)\))\s*{BLOCK_REGEX}"
)
IFDEF_ELSE_REGEX = re.compile(_IFDEF_REGEX + rf"\s*ELSE\s*{BLOCK_REGEX}")
IFDEF_REGEX = re.compile(_IFDEF_REGEX)


def _process_ifdefs(
    regex: re.Pattern, html: str, kwargs: typing.Dict[str, str]
) -> str:
    while match := re.search(regex, html):
        cond = match.group("cond")
        kwarg = match.group("arg")

        if_block, else_block, full = read_ifdef_block(match.start(), html)

        if (cond == "IFDEF" and kwarg in kwargs) or (
            cond == "IFNDEF" and kwarg not in kwargs
        ):
            repl = block_contents(if_block).strip()
        else:
            repl = block_contents(else_block).strip()

        html = html.replace(full, repl, 1)
    return html


def process_ifdefs(html: str, kwargs: typing.Dict[str, str]) -> str:
    html = _process_ifdefs(IFDEF_ELSE_REGEX, html, kwargs)
    html = _process_ifdefs(IFDEF_REGEX, html, kwargs)
    return html


KWARG_REGEX = re.compile(rf"{OPEN}\s*(?P<k>[A-Z_]+)\s*{CLOSE}")


def process_kwarg_html(html: str, kwargs: typing.Dict[str, str]) -> str:
    html = process_preamble(html, kwargs)
    html = process_ifdefs(html, kwargs)
    for kwarg in re.finditer(KWARG_REGEX, html):
        html = html.replace(kwarg.group(0), kwargs.get(kwarg.group("k"), ""))

    return process_html(html)


QUOTE_CHARS = "\"'|"


def remove_quotes(string: str) -> str:
    if string[0] in QUOTE_CHARS:
        return string[1:-1]
    return string


def kwarg_substitution(html: str, args: str = "") -> str:
    kwargs = {
        k.upper(): remove_quotes(v)
        for k, v in map(
            lambda arg: re.split(r"\s*=\s*", arg, 1),
            [
                term.group(0)
                for term in re.finditer(KWARG_ARG_REGEX, args.strip())
            ],
        )
    }

    try:
        return process_kwarg_html(html, kwargs)
    except ValueError as e:
        print(f"Substitution failed.\nReason: {e}\nArgs: {kwargs}")
        exit(os.EX_DATAERR)


def kwarg_file_subsitution(file: str, args: str = "") -> str:
    html = include_file(file, False)
    return kwarg_substitution(html, args)


def html_file_substitution(tag: str, args: str = "") -> str:
    # </Form> means <FormEnd>
    if tag[0] == "/":
        processed = tag[1:] + "End"
    else:
        processed = tag

    # Convert from camelcase, e.g.
    # FormField to form/field or form_field
    parts = []
    part = ""
    for c in processed:
        if c.isupper():
            if part:
                parts.append(part)
            part = c.lower()
        else:
            part += c
    if part:
        parts.append(part)

    path = "/".join(parts)
    start = path + "/start"
    snake = "_".join(parts)

    for file in [path, start, snake]:
        try:
            html = include_file(file, False)
            break
        except (FileNotFoundError, IsADirectoryError):
            pass
    else:
        print(f"Missing include file for tag: {tag}")
        exit(os.EX_DATAERR)

    return kwarg_substitution(html, args)


def include_file(file: str, process: bool = True) -> str:
    try:
        include = os.path.join(INCLUDE_DIR, file)
        with open(include, "r") as f:
            html = f.read()
    except (FileNotFoundError, IsADirectoryError):
        # Allow omission of file extension
        include = os.path.join(INCLUDE_DIR, file + ".html")
        with open(include, "r") as f:
            html = f.read()

    # Note: This could recurse until OOM if a file is self-referential.
    # Don't do that.
    if process:
        return process_html(html)
    else:
        return html


def handle_match(match: re.Match) -> str:
    if kw_file := match.group("kwarg_file"):
        return kwarg_file_subsitution(kw_file, match.group("args"))
    elif ht_file := match.group("html_file"):
        return html_file_substitution(ht_file, match.group("html_args"))
    elif func := match.group("func"):
        return function_substitution(func, match.group("arg"))
    elif file := match.group("file"):
        return include_file(file)
    elif text := match.group("raw"):
        return text
    else:
        # This path is unreachable unless additional options are added to the
        # regular expression.
        return ""


def process_html(html: str) -> str:
    while match := re.search(SUBSTITUTION_REGEX, html):
        html = html.replace(match.group(0), handle_match(match))
    return html


def process_page(page) -> None:
    output_dir = output_directory()
    with open(os.path.join(PAGES_DIR, page), "r") as f:
        html = f.read()

    html = process_html(html)
    if OPEN in html:
        print(f"Substitution may have failed for {page}.")

    with open(os.path.join(output_dir, page), "w") as f:
        f.write(html)


PROCESSES = 12
MULTIPROCESSING = False


def main() -> None:
    if MULTIPROCESSING:
        with multiprocessing.Pool(PROCESSES) as pool:
            pool.map(process_page, os.listdir(PAGES_DIR))
    else:
        for page in os.listdir(PAGES_DIR):
            process_page(page)


if __name__ == "__main__":
    main()
