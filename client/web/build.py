import os
import re
import sys

script_dir = os.path.abspath(os.path.dirname(__file__))
include_dir = os.path.join(script_dir, "include")
pages_dir = os.path.join(script_dir, "pages")
if len(sys.argv) > 1:
    output_dir = sys.argv[1]
else:
    output_dir = os.path.join(script_dir, "output")

for page in os.listdir(pages_dir):
    with open(os.path.join(pages_dir, page), "r") as f:
        html = f.read()

    for match in re.finditer(r"{{(?P<include>[^{}]*)}}", html):
        try:
            file_name = match.group("include").strip()
            include = os.path.join(include_dir, file_name)
            with open(include, "r") as f:
                html = html.replace(match.group(0), f.read())
        except FileNotFoundError:
            print(f"Missing include file {file_name}. Aborting.")
            exit()

    with open(os.path.join(output_dir, page), "w") as f:
        f.write(html)
