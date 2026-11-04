"""Semantic HTML tree for the watch reader. No script, CSS evaluator or browser process."""
from html.parser import HTMLParser
import re
from urllib.parse import urljoin, urlsplit


class WebDocument(HTMLParser):
    VOID = {"area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"}
    def __init__(self, url):
        super().__init__(convert_charrefs=True)
        self.url, self.base = url, url
        self.blocks, self.anchors, self.buffer, self.title = [], {}, [], []
        self.hidden, self.links = [], []
        self.heading, self.bold, self.code, self.list_depth = 0, 0, 0, 0
        self.in_title, self.script = False, False

    def flush(self):
        text = "".join(self.buffer)
        self.buffer = []
        text = text.expandtabs(2).strip() if self.code else re.sub(r"\s+", " ", text).strip()
        if not text:
            return
        kind = "heading" if self.heading else "code" if self.code else "paragraph"
        for at in range(0, len(text), 900):
            if len(self.blocks) >= 512:
                break
            self.blocks.append({"type": kind, "text": text[at:at+900], "level": self.heading,
                                "bold": self.bold > 0, "indent": min(self.list_depth, 3)})

    def target(self, value):
        try:
            value = urljoin(self.base, value.strip())
            parsed = urlsplit(value)
        except ValueError:
            return ""
        return value if parsed.scheme == "https" and not parsed.username and not parsed.password and len(value) <= 2048 else ""

    def handle_starttag(self, tag, attrs):
        attrs = {key: value or "" for key, value in attrs}
        if tag == "title":
            self.in_title = True
        if tag == "script":
            self.script = True
        if tag == "base" and attrs.get("href"):
            self.base = self.target(attrs["href"]) or self.base
        if self.hidden:
            if tag not in self.VOID:
                self.hidden.append(tag)
            return
        style = re.sub(r"\s+", "", attrs.get("style", "").lower())
        if tag in {"head", "script", "style", "svg", "iframe", "object", "form", "template"} or "hidden" in attrs or "display:none" in style or "visibility:hidden" in style:
            if tag not in self.VOID:
                self.hidden.append(tag)
            return
        if attrs.get("id") or (tag == "a" and attrs.get("name")):
            self.flush()
            self.anchors[attrs.get("id") or attrs["name"]] = len(self.blocks)
        if tag in {"h1", "h2", "h3", "h4", "h5", "h6"}:
            self.flush()
            self.heading = int(tag[1])
        elif tag in {"p", "div", "section", "article", "main", "header", "footer", "nav", "br", "tr", "blockquote"}:
            self.flush()
            if tag == "blockquote":
                self.buffer.append("> ")
        elif tag in {"ul", "ol"}:
            self.flush()
            self.list_depth += 1
        elif tag == "li":
            self.flush()
            self.buffer.append("- ")
        elif tag in {"td", "th"}:
            self.buffer.append(" | " if self.buffer else "")
        elif tag in {"strong", "b"}:
            self.flush()
            self.bold += 1
        elif tag == "pre":
            self.flush()
            self.code += 1
        elif tag == "a":
            self.flush()
            self.links.append((self.target(attrs["href"]) if attrs.get("href") else "", []))
        elif tag == "hr":
            self.flush()
            if len(self.blocks) < 512:
                self.blocks.append({"type": "rule", "text": ""})
        elif tag == "img":
            self.flush()
            source = attrs.get("src", "")
            if source.startswith("data:image/"):
                url = source if len(source) <= 300000 else ""
            else:
                url = self.target(source) if source else ""
            if url and attrs.get("width") != "1" and attrs.get("height") != "1" and len(self.blocks) < 512:
                self.blocks.append({"type": "image", "source": url, "text": attrs.get("alt", "Изображение")[:120]})

    def handle_endtag(self, tag):
        if tag == "title":
            self.in_title = False
        if self.hidden:
            if tag in self.hidden:
                self.hidden = self.hidden[:len(self.hidden) - 1 - self.hidden[::-1].index(tag)]
            return
        if tag == "a" and self.links:
            target, text = self.links.pop()
            label = re.sub(r"\s+", " ", "".join(text)).strip()
            if label and len(self.blocks) < 512:
                self.blocks.append({"type": "link" if target else "paragraph", "text": label[:900], "target": target})
        elif tag in {"h1", "h2", "h3", "h4", "h5", "h6"}:
            self.flush()
            self.heading = 0
        elif tag in {"p", "div", "li", "section", "article", "main", "header", "footer", "nav", "tr", "blockquote"}:
            self.flush()
        elif tag in {"ul", "ol"}:
            self.flush()
            self.list_depth = max(0, self.list_depth - 1)
        elif tag in {"strong", "b"}:
            self.flush()
            self.bold = max(0, self.bold - 1)
        elif tag == "pre":
            self.flush()
            self.code = max(0, self.code - 1)

    def handle_data(self, data):
        if self.in_title:
            self.title.append(data)
        if self.hidden:
            return
        if self.links:
            self.links[-1][1].append(data)
        else:
            self.buffer.append(data)


def parse_document(html, url):
    parser = WebDocument(url)
    parser.feed(html[:262144])
    parser.close()
    parser.flush()
    return {"title": " ".join("".join(parser.title).split())[:120], "blocks": parser.blocks,
            "anchors": parser.anchors, "needs_js": parser.script and sum(len(b.get("text", "")) for b in parser.blocks) < 60}
