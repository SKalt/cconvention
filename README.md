# `conventional-commit-language-server`

A language server to help write [conventional commits][ccs].

`git commit` on the command-line opens a [`${GIT_DIR}/COMMIT_EDITMSG`](https://git-scm.com/docs/git-commit#_files) file in your `$EDITOR` of choice.
`conventional-commit-language-server` acts as a language server, providing completion, linting, and formatting.

Pairs well with [`git-cc`][git-cc], a <abbr title="Terminal User Interface">TUI</abbr> for writing conventional commits.

<!-- TODO: ## Usage
  TODO: command-line usage (automate with cog)
  TODO: pre-commit
  TODO: vscode
  TODO: vim
  TODO: emacs
  TODO: helix
  TODO: sublime
  TODO: jetbrains
-->

<!-- TODO: ## Installation
  TODO: curl | sh
  TODO: deb
  TODO: nix
  TODO: rpm
  TODO: apk
  TODO: pypi
  TODO: npm
  TODO: brew
  TODO: gem
-->

## Licensing

This tool comes in two variants -- an open-source edition which you can build upon and use for any purpose, and a source-provided version that

> To use [this software] to make money or for work, you need to buy a license.
> If you’re part of a team, everyone on your team who uses the software needs to buy one.
> You can [try before you buy](./pkg/pro/LICENSE.md#free-trials), to make sure the software works and integrates well with your prototypes.

|            Feature             |        OSS        |       Source-provided       |
| :----------------------------: | :---------------: | :-------------------------: |
|            License             | [Apache-2.0][oss] | [indiecc-4.0][src-provided] |
| error & performance monitoring |    opt-**out**    |         opt-**in**          |
|       configuration file       |       ❌ no       |           ✅ yes            |

- OSS:
  - `pkg/base/`
  - `editors/*/base`
- Source-provided:
  - `pkg/pro/`
  - `editors/*/pro`

<!-- TODO: ## Roadmap -->

<!-- links -->

[ccs]: https://conventionalcommits.org
[git-cc]: https://github.com/skalt/git-cc
[oss]: ./pkg/base/LICENSE.md
[src-provided]: ./pkg/pro/LICENSE.md
