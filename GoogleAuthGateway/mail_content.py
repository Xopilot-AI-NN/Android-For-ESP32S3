#!/usr/bin/env python3
"""Small mail renderer worker. Input/output stay in pipes; email contents are never logged."""
import base64
from html.parser import HTMLParser
import io
import json
import re
import sys
import warnings
import zlib


class MailHTML(HTMLParser):
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.blocks, self.text, self.hidden, self.links = [], [], [], []

    def flush(self):
        text = re.sub(r"[ \t\r\f\v]+", " ", "".join(self.text))
        text = re.sub(r" *\n *", "\n", text)
        text = re.sub(r"\n{3,}", "\n\n", text).strip()
        if text:
            self.blocks.append({"type": "text", "text": text})
        self.text = []

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if self.hidden:
            if tag not in {"br", "img", "hr", "input", "meta", "link", "source", "wbr"}:
                self.hidden.append(tag)
            return
        style = re.sub(r"\s+", "", attrs.get("style", "").lower())
        if tag in {"head", "script", "style", "svg", "iframe", "object", "form", "noscript"} or "hidden" in attrs or "display:none" in style or "visibility:hidden" in style:
            self.hidden.append(tag)
            return
        if tag == "img":
            source = attrs.get("src", "")
            # Tracking pixels have no useful visual content on the watch.
            if attrs.get("width") == "1" or attrs.get("height") == "1":
                return
            self.flush()
            if len(source) <= 3 * 1024 * 1024 and sum(b["type"] == "image" for b in self.blocks) < 64:
                self.blocks.append({"type": "image", "source": source, "alt": attrs.get("alt", "")[:120]})
        elif tag in {"p", "div", "br", "tr", "h1", "h2", "h3", "h4", "blockquote", "hr"}:
            self.text.append("\n")
        elif tag == "li":
            self.text.append("\n• ")
        elif tag in {"td", "th"}:
            self.text.append(" · ")
        elif tag == "a":
            href = attrs.get("href", "")
            self.links.append(href if href.startswith(("https://", "mailto:")) and len(href) <= 512 else "")

    def handle_endtag(self, tag):
        if self.hidden:
            if tag in self.hidden:
                self.hidden = self.hidden[:len(self.hidden) - 1 - self.hidden[::-1].index(tag)]
            return
        if tag in {"p", "div", "li", "tr", "h1", "h2", "h3", "h4", "blockquote"}:
            self.text.append("\n")
        elif tag == "a" and self.links:
            href = self.links.pop()
            if href:
                self.text.append(" (" + href + ")")

    def handle_data(self, data):
        if not self.hidden:
            self.text.append(data)


class SearchHTML(HTMLParser):
    """Read result cards from DuckDuckGo's non-JavaScript page, never navigation/ad links."""
    def __init__(self):
        super().__init__(convert_charrefs=True)
        self.depth = 0
        self.current = None
        self.capture = None
        self.results = []
        self.no_results = False

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag not in {"br", "img", "hr", "meta", "link", "input"}:
            self.depth += 1
        classes = set(attrs.get("class", "").split())
        if any(value.startswith("no-results") for value in classes):
            self.no_results = True
        if tag == "div" and "result" in classes and self.current is None:
            self.current = {"depth": self.depth, "url": "", "title": [], "description": [],
                            "ad": "result--ad" in classes}
        if self.current:
            if tag == "a" and "result__a" in classes:
                self.current["url"] = attrs.get("href", "")
                self.capture = ("title", self.depth)
            elif "result__snippet" in classes:
                self.capture = ("description", self.depth)

    def handle_endtag(self, tag):
        if self.capture and self.capture[1] == self.depth:
            self.capture = None
        if self.current and tag == "div" and self.current["depth"] == self.depth:
            row = self.current
            if row["url"] and not row["ad"] and len(self.results) < 10:
                self.results.append({"url": row["url"],
                    "title": " ".join("".join(row["title"]).split())[:160],
                    "description": " ".join("".join(row["description"]).split())[:400]})
            self.current = None
        if tag not in {"br", "img", "hr", "meta", "link", "input"}:
            self.depth = max(0, self.depth - 1)

    def handle_data(self, data):
        if self.current and self.capture:
            self.current[self.capture[0]].append(data)


def main():
    request = json.load(sys.stdin)
    if request["operation"] == "browser":
        from web_document import parse_document
        json.dump(parse_document(request["html"], request["url"]), sys.stdout, ensure_ascii=False)
        return
    if request["operation"] == "search":
        parser = SearchHTML()
        parser.feed(request["html"][:400000])
        parser.close()
        json.dump({"results": parser.results, "no_results": parser.no_results}, sys.stdout, ensure_ascii=False)
        return
    if request["operation"] == "html":
        parser = MailHTML()
        parser.feed(request["html"][:2 * 1024 * 1024])
        parser.close()
        parser.flush()
        json.dump(parser.blocks, sys.stdout, ensure_ascii=False)
        return
    from PIL import Image, ImageOps
    # Decode in a short-lived process: malformed/decompression-bomb images never reach ESP RAM.
    Image.MAX_IMAGE_PIXELS = 4_000_000
    warnings.simplefilter("error", Image.DecompressionBombWarning)
    with Image.open(io.BytesIO(base64.b64decode(request["data"], validate=True))) as original:
        if original.format not in {"JPEG", "PNG", "GIF", "WEBP"}:
            raise ValueError("unsupported image")
        image = ImageOps.exif_transpose(original).convert("RGBA")
        image.thumbnail((108, 80), Image.Resampling.LANCZOS)
        background = Image.new("RGBA", image.size, "white")
        background.alpha_composite(image)
        raw = bytearray()
        for r, g, b in background.convert("RGB").getdata():
            value = ((r >> 3) << 11) | ((g >> 2) << 5) | (b >> 3)
            raw.extend(value.to_bytes(2, "little"))
        compressed = zlib.compress(raw, 9)
        json.dump({"width": image.width, "height": image.height, "codec": "rgb565-zlib",
                   "data": base64.b64encode(compressed).decode("ascii")}, sys.stdout)


if __name__ == "__main__":
    try:
        main()
    except Exception:
        # No tracebacks: parser exceptions can contain pieces of a private email.
        sys.exit(1)
