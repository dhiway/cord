# Canonical source header policy

All tracked program source files use `HEADER-GPL3` as the single license-header source of truth.
Rust, TypeScript, JavaScript, and code-generation templates carry the file verbatim. Python, shell,
and Docker sources carry the same content with `#` comment markers, after a required shebang.

The policy applies to tracked source and generated source. JSON, TOML, YAML, lockfiles, binary
fixtures, evidence, Markdown, and other data/document formats that cannot safely carry a program
comment are not source-header targets.

Run:

```sh
python3 scripts/check-source-headers.py
```

Use `--fix` only for an intentional repository-wide normalization. Generators must emit the
canonical header themselves; CI must never rely on `--fix`.
