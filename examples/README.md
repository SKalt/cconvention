# Examples

```
.
├── example.expected
└── example.msg
```

Each `example.msg` file should demonstrate zero or more errors.
Each `example.expected` file should demonstrate what `cconvention check -f example.msg` would output.

Note the .msg extension -- used to prevent IDEs running `cconvention` from auto-formatting the malformed commit messages.
