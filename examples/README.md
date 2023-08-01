# Examples

```
.
├── example.expected
└── example.msg
```

Each `example.msg` file should demonstrate zero or more errors.
Each `example.expected` file should demonstrate what `conventional-commit-language-server check -f example.msg` would output.

Note the .msg extension -- used to prevent IDEs running `conventional-commit-language-server` from auto-formatting the malformed commit messages.
