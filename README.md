# `cconvention`

A language server to help write [conventional commits][ccs].

`git commit` on the command-line opens a [`${GIT_DIR}/COMMIT_EDITMSG`](https://git-scm.com/docs/git-commit#_files) file in your `$EDITOR` of choice.
`cconvention` acts as a language server to provide completion, linting, and formatting.

Pairs well with [`git-cc`][git-cc], a <abbr title="Terminal User Interface">TUI</abbr> for writing conventional commits.

### Warning: this is alpha software

Any part of the public-API may change with little or no warning until the first major-version release.
This includes the names of any published tools, libraries, binaries, or extensions.

<!--
  roadmap:
    TODO: make own tree-sitter grammar?
    TODO: settle on stable name: cconvention?
-->

## Command-line Usage

```sh
cconvention --help | sed 's/^/# /g'
# Usage: cconvention <COMMAND>
#
# Commands:
#   serve  Run a language server
#   check  Lint commit message(s)
#   help   Print this message or the help of the given subcommand(s)
#
# Options:
#   -h, --help     Print help
#   -V, --version  Print version
```

<!--
  TODO: automate IDE usage docs with cog
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

This tool comes in two variants -- an open-source edition which you can build upon and use for any purpose, and a source-provided version that is governed by a source-provided license.
Summarized, the source-provided license states:

> To use [this software] to make money or for work, you need to buy a license.
> If you’re part of a team, everyone on your team who uses the software needs to buy one.
> You can [try before you buy](./licenses/LICENSE.indiecc-4.md#free-trials), to make sure the software works and integrates well with your prototypes.

|             Feature              |        OSS        |       Source-provided       |
| :------------------------------: | :---------------: | :-------------------------: |
|             License              | [Apache-2.0][oss] | [indiecc-4.0][src-provided] |
|  error & performance monitoring  |    opt-**out**    |         opt-**in**          |
|        configuration file        |       ❌ no       |           ✅ yes            |
| ability to write your own checks |       ❌ no       |           ✅ yes            |

- OSS:
  - `pkg/base/`
  - `editors/*/base`
- Source-provided:
  - `pkg/pro/`
  - `editors/*/pro`

If you're ever confused which license applies, check the closest LICENSE.md up the directory tree.

<!-- links -->

[ccs]: https://conventionalcommits.org
[git-cc]: https://github.com/skalt/git-cc
[oss]: ./pkg/base/LICENSE.md
[src-provided]: ./licenses/LICENSE.indiecc-4.md
