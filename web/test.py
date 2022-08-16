import typing
import unittest

import build


class Test(unittest.TestCase):
    def test_ifdefs(self) -> None:
        html = "IFDEF(VAR) {{ yes }} ELSE {{ no }}"
        self.assertEqual(build.process_ifdefs(html, {"VAR": ""}), "yes")
        self.assertEqual(build.process_ifdefs(html, {}), "no")

    def test_ifdef_multiline(self) -> None:
        html = (
            "IFDEF(MULTI)\n\t{{\nline one\nline two\n}}\nELSE\n\r{{one line}}"
        )
        self.assertEqual(
            build.process_ifdefs(html, {"MULTI": ""}), "line one\nline two"
        )
        self.assertEqual(build.process_ifdefs(html, {}), "one line")

    def test_kwarg_substitution(self) -> None:
        html = "IFNDEF(VAR) {{ Hello World }} ELSE {{ {{ VAR }} }}"
        self.assertEqual(build.process_kwarg_html(html, {}), "Hello World")
        phrase = "Goodbye World"
        self.assertEqual(
            build.process_kwarg_html(html, {"VAR": phrase}), phrase
        )

    def test_preamble(self) -> None:
        kwargs: typing.Dict[str, str] = {}
        html = "PREAMBLE {{\nkwargs['VAR'] = 'val'\n}}"
        build.process_preamble(html, kwargs)
        self.assertEqual(kwargs["VAR"], "val")


if __name__ == "__main__":
    unittest.main()
