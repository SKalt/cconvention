# `cconvention`

A language server to help write [conventional commits][ccs].

`git commit` on the command-line opens a [`${GIT_DIR}/COMMIT_EDITMSG`](https://git-scm.com/docs/git-commit#_files) file in your `$EDITOR` of choice.
`cconvention` acts as a language server to provide completion, linting, and formatting.

Pairs well with [`git-cc`][git-cc], a <abbr title="Terminal User Interface">TUI</abbr> for writing conventional commits.

### Warning: this is alpha software

Any part of the public-API may change with little or no warning until the first major-version release.
This includes the names of any published tools, libraries, binaries, or extensions.

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

`cconvention` comes in flavors -- an open-source (OSS) edition which you can build upon and use for any purpose, and a source-provided version with commercial use restrictions.
Summarized, the source-provided license states:

> To use `cconvention` to make money or for work, you need to buy a license.
> You can try before you buy for a month to make sure the software works for you.

|             Feature              |        OSS        |                      Source-provided                      |
| :------------------------------: | :---------------: | :-------------------------------------------------------: |
|             License              | [Apache-2.0][oss] | [noncommercial OR free-trial OR COMMERCIAL][src-provided] |
|  error & performance monitoring  |    opt-**out**    |                        opt-**in**                         |
|        configuration file        |       ❌ no       |                          ✅ yes                           |
| ability to write your own checks |       ❌ no       |                          ✅ yes                           |

The OSS editions are located in:

- `pkg/base/`
- `editors/*/base`

The source-provided editions are located in:

- `pkg/pro/`
- `editors/*/pro`

If you're ever confused which license applies, check the nearest file header.

<!-- links -->

[ccs]: https://conventionalcommits.org
[git-cc]: https://github.com/skalt/git-cc
[oss]: ./LICENSES/APACHE-2.0.md
[src-provided]: ./editors/code/pro/LICENSE.md
