# Contributing to NfoGlow for Windows

Thanks for wanting to help. Bug reports, ideas and code are all welcome.

- **Found a bug or have an idea?** Open an issue with one of the templates. Please search the existing issues first. For a display problem, attach the `.nfo` that shows it if you can.
- **A question, or something that is not a bug?** Start a discussion.
- **Translations** come from NfoGlow for Linux: please send corrections there, with its "Translation fix" form or a pull request on its `po/*.po` files. Both apps get them from there.
- **Code:** please open an issue before a big change, so we agree on the direction first. Small fixes can go straight to a pull request.

NfoGlow for Linux is the reference: what an NFO looks like, the effects and the options should match it (see [CONFORMANCE.md](CONFORMANCE.md)). The window itself follows the style of Windows 11.

## Before you send a pull request

How the project is laid out and how to build and run it is in [docs/development.md](docs/development.md). In short, these must pass (CI runs the tests and the build on Windows):

```sh
cargo test
cargo clippy
cargo build --release
```

Please keep one change per pull request and write commit messages as [Conventional Commits](https://www.conventionalcommits.org/) (`fix: ...`, `feat: ...`, `docs: ...`).

By contributing you agree that your work is published under the project's license (GPL-3.0-or-later).
