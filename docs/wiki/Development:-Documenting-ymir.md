ymir's documentation files are found in `docs/wiki/` and should be viewable and browsable in at least two systems:

- The repository's markdown file preview
- [The documentation site](https://lab.braxton.onl/braxton/ymir/)

## The documentation site

The documentation site is generated with [mkdocs](https://www.mkdocs.org/).
The configuration files are found in `docs/`.

To set up and run the documentation site locally, it is recommended to use [uv](https://docs.astral.sh/uv/).

### Serving the site locally with uv

In the `docs/` subdirectory:

- `uv sync`
- `uv run mkdocs serve`

The documentation site should now be available on http://127.0.0.1:8000/

Changes made to the documentation while the development server is running will cause an automatic page refresh in the browser.

> [!TIP]
> Images may not be visible, as they are stored on Git LFS.
> If this is the case, run `git lfs pull`.

## Elements

Elements such as links, admonitions, images, and snippets should work as expected in the markdown file preview and in the documentation site.

### Links

Links should in all cases be relative (e.g. `./FAQ.md`), unless it's an external one.
Links should have anchors if they are meant to lead the user to a specific section on a page (e.g. `./Getting-Started.md#nvidia`).

> [!TIP]
> mkdocs will terminate if relative links lead to non-existing documents or non-existing anchors.
> This means that the CI pipeline will fail when building documentation, as will `mkdocs serve` locally.

### Admonitions

> [!IMPORTANT]
> This is an important distinction from other `mkdocs`-based documentation you might have encountered.

Admonitions, or alerts should be written [the way GitHub defines them](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts).
The above admonition is written like this:

```
> [!IMPORTANT]
> This is an important distinction from other `mkdocs`-based documentation you might have encountered.
```

### Images

Images should have relative links to resources in `docs/wiki/img/`, and should contain sensible alt-text.

### Videos

Videos need to be wrapped in a `<video>` tag (displayed by mkdocs) and have the video link again as fallback text, padded with blank lines:

```html
<video controls src="https://example.org/assets/video.mp4">

https://example.org/assets/video.mp4

</video>
```

### Snippets

Configuration and code snippets in general should be annotated with a language.

If the language used in the snippet is Lua, open the code block like this:

```md
```lua
```