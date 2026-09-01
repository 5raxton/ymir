ymir's documentation is hosted in the built-in [Forgejo wiki](https://lab.braxton.onl/braxton/ymir/), which is a separate Git repository: `braxton/ymir.wiki.git`.

To work on the documentation locally:

```sh
fj wiki clone   # or: git clone git@lab.braxton.onl:braxton/ymir.wiki.git
```

Each Markdown file (`.md`) in the wiki repository corresponds to a wiki page. Pages use relative links between themselves, and supporting assets (images, logos, examples) live alongside them in the same repository.

> [!TIP]
> Images may not be visible, as they are stored on Git LFS.
> If this is the case, run `git lfs pull`.

## Elements

Elements such as links, admonitions, images, and snippets should work as expected in the rendered wiki.

### Links

Links should in all cases be relative (e.g. `./FAQ.md`), unless it's an external one.
Links should have anchors if they are meant to lead the user to a specific section on a page (e.g. `./Getting-Started.md#nvidia`).

### Admonitions

> [!IMPORTANT]
> Admonitions, or alerts, should be written [the way GitHub defines them](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts).

The above admonition is written like this:

```
> [!IMPORTANT]
> Admonitions, or alerts, should be written [the way GitHub defines them](https://docs.github.com/en/get-started/writing-on-github/getting-started-with-writing-and-formatting-on-github/basic-writing-and-formatting-syntax#alerts).
```

### Images

Images should have relative links to resources in `img/`, and should contain sensible alt-text.

### Videos

Videos need to be wrapped in a `<video>` tag and have the video link again as fallback text, padded with blank lines:

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
