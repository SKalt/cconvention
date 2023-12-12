# `git-conventional-commit-ls-basic`

Provides completion, linting, and formatting of [conventional commits][ccs].

`git commit` on the command-line opens a [`${GIT_DIR}/COMMIT_EDITMSG`](https://git-scm.com/docs/git-commit#_files) file in your `$EDITOR` of choice.
`cconvention` acts as a language server to provide completion, linting, and formatting.

<!-- TODO: embed video -->

Powered by the [`cconvention`][repo]'s base language server.
Pairs well with [`git-cc`][git-cc], a <abbr title="Terminal User Interface">TUI</abbr> for writing conventional commits.

## Usage

First, make sure that `code` is available on the command line:

```sh
command -v code || echo 'code needs to be set up on the command line'
```

If `code` is missing, run "**Shell Command: Install 'code' command in PATH**" from the VScode command palette.

Then, configure `git` to write git commit messages with `code`.
You can do this several ways:

1. (**recommended**) Using command-line git:
   ```sh
   # in your repo
   git config \
    core.editor 'code --wait'
   # or globally:
   git config --global \
    core.editor 'code --wait'
   ```
1. By editing `${GIT_DIR}/config` or your global `.gitconfig` file:

   ```ini
   [core]
       editor = code --wait
   ```

1. Running
   ```sh
   export EDITOR='code --wait'
   ```
   in your current shell or adding the command to a file that consistently gets run during your shell's lifecycle, such as [`.envrc`][direnv] or `~/.profile`.

Now, when you run `git commit`, `code` will open `${GIT_DIR}/COMMIT_EDITMSG` and start the language server.

<!-- TODO: note about telemetry &/ alert + consent -->
<!-- TODO: notes about configuration in pro version -->

<!-- links -->

[ccs]: https://conventionalcommits.org
[repo]: https://github.com/skalt/cconvention
[git-cc]: https://github.com/skalt/git-cc
[direnv]: https://direnv.net/
