import os
import re
import sys
import urllib3

from typing import Tuple


def directories() -> Tuple[str, str, str]:
    script_dir = os.path.abspath(os.path.dirname(__file__))
    include_dir = os.path.join(script_dir, "include")
    pages_dir = os.path.join(script_dir, "pages")
    if len(sys.argv) > 1:
        output_dir = sys.argv[1]
    else:
        output_dir = os.path.join(script_dir, "output")

    return pages_dir, include_dir, output_dir


def substitution_regex() -> re.Pattern:
    ic = r"[a-zA-Z_.\-]"
    function_regex = rf"(?P<func>{ic}*)\((?P<arg>{ic}+)\)"
    include_regex = rf"(?P<file>{ic}*)"
    overall_regex = r"{{ *(" + function_regex + r"|" + include_regex + r") *}}"

    return re.compile(overall_regex)


def bootstrap_icon(name: str) -> str:
    URL_FORMAT = "https://icons.getbootstrap.com/assets/icons/{}.svg"

    url = URL_FORMAT.format(name)
    resp = urllib3.PoolManager().request("GET", url)
    if resp.status != 200:
        print(f"Failed to retrieve {url} status {resp.status}.")
        exit(os.EX_DATAERR)
    return resp.data.decode("utf-8")


def handle_match(match: re.Match, include_dir: str) -> str:
    if func := match.group("func"):
        functions = {f.__name__: f for f in [bootstrap_icon]}
        try:
            return functions[func](match.group("arg"))
        except KeyError:
            print(f"Missing function: {func}. Aborting.")
            exit(os.EX_NOINPUT)
    elif file := match.group("file"):
        try:
            include = os.path.join(include_dir, file)
            with open(include, "r") as f:
                return f.read()
        except FileNotFoundError:
            print(f"Missing include file: {file}. Aborting.")
            exit(os.EX_DATAERR)
    else:
        # This path is unreachable unless additional options are added to the
        # regular expression.
        return ""


def process_html(html: str, include_dir: str) -> str:
    regex = substitution_regex()
    for match in re.finditer(regex, html):
        print(match.group(0))
        html = html.replace(match.group(0), handle_match(match, include_dir))
    return html


def main() -> None:
    pages_dir, include_dir, output_dir = directories()

    for page in os.listdir(pages_dir):
        with open(os.path.join(pages_dir, page), "r") as f:
            html = f.read()

        html = process_html(html, include_dir)

        with open(os.path.join(output_dir, page), "w") as f:
            f.write(html)


if __name__ == "__main__":
    main()
