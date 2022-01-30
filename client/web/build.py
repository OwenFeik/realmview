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
    ic = r"a-zA-Z0-9_.\-"
    function_regex = rf"(?P<func>[{ic}]*)\(\"?(?P<arg>[{ic}:/@]+)\"?\)"
    include_regex = rf"(?P<file>[{ic}]*)"
    overall_regex = r"{{ *(" + function_regex + r"|" + include_regex + r") *}}"

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


def stylesheet(url: str) -> str:
    return f"<style>{load_url(url)}</style>"


def javascript(url: str) -> str:
    return f"<script>{load_url(url)}</script>"


def function_substitution(func: str, arg: str) -> str:
    functions = {
        f.__name__: f for f in [bootstrap_icon, stylesheet, javascript]
    }
    try:
        return functions[func](arg)
    except KeyError:
        print(f"Missing function: {func}. Aborting.")
        exit(os.EX_NOINPUT)


def file_substitution(file: str) -> str:
    try:
        include = os.path.join(INCLUDE_DIR, file)
        with open(include, "r") as f:
            # Note: This could recurse until OOM if a file is self-referential.
            return process_html(f.read())
    except FileNotFoundError:
        print(f"Missing include file: {file}. Aborting.")
        exit(os.EX_DATAERR)


def handle_match(match: re.Match) -> str:
    if func := match.group("func"):
        return function_substitution(func, match.group("arg"))
    elif file := match.group("file"):
        return file_substitution(file)
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

        with open(os.path.join(output_dir, page), "w") as f:
            f.write(html)


if __name__ == "__main__":
    main()
