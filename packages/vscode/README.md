# Inkpot

A plaintext format for writing novels.

## Features

- Markdown-like syntax highlighting for `.inkpot` and `.ip` documents
- Context tag highlighting for marking scenes as `@todo`, `@wip`, `@draft` and `@done`
- Hierarchical headers with visible (`## Header`) and non-visible (`== Header`) options
- Scene synopses describing the written contents of a scene; just start a line with `~`
- Header tags and cookies for recording novel info: `[1/3]` and `words:123`
- Dated headers for recording daily word counts; just start your header with a date (`2025-02-01`)

## Example

```
---
Title: Novel
Author: Thom Bruce
Date: 2025-02-01
Copyright: (C) 2025 Thom Bruce
Draft date: 2025-02-01
---

~ This document represents a novel outline

= Act I [0/2]

~ Synopses can be used to describe the contents of any headered section

## Chapter 1 @wip [0/2] words:52

~ This one follows a visible chapter marker

=== Scene 1 @wip words:52

~ This is an example **synopsis**, _hell, yeah_! ~~Ignore this.~~

==== 2025-02-01 words:26

This is an idea to record daily word counts within the document itself...

It could use cookies or it could use tags as currently demonstrated above.

==== 2025-02-02 words:26

This is an idea to record daily word counts within the document itself...

It could use cookies or it could use tags as currently demonstrated above.

=== Scene 2 @todo

~ This is another synopsis!

This ~~secne~~ scene demonstrates the use of **Mardown** to render _inline_ `markup`.

/ It also features a comment.

## Chapter 2: _Headings Still Support Markdown_ @todo [0/0] words:0
```

## Known Issues

- It's just a language syntax highlighter for now; richer features are yet to come.

## Release Notes

### 0.2.0

Extend support to full Markdown subset

### 0.1.0

Initial release of Inkpot
