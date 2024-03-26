# Zig Manager

> Currently only `default` command with Linux and MacOS done. I'm rewriting in Zig ASAP : )

## Usage
`<VERSION>` can be a version number like `0.12.0`, `latest` or `master`.

`zman default [OPTIONS] <VERSION>`: Download and set a Zig version as default. Also adds shims like zig-cc and zig-c++
```bash
zman default latest
zman default master
zman default 0.12.0
```

Options are:
```bash
--install <DIR> # Set the install directory. By default installs to $HOME/.local/share/zman
--link <DIR> # Set the path to link the binaries to. By default links to $HOME/.local/bin
--no-dropins # Do not create shims like `zig-cc` or `zig-c++` for Zig drop-in replacements 
```

`zman fetch [OPTIONS] <VERSION>`: Only downloads a zig version

`zman clean [VERSION]`: To clean every version of Zig, except `default` and `master` or, provide a version to clean only that particular version
```bash
zman clean
zman clean latest
zman clean master
zman clean 0.12.0
```
`zman list`: List all installed versions

`zman keep <VERSION>`: Prevent a version from being cleaned by `zman clean`. Can be reverted by running clean the specific version
```bash
zman keep 0.12.0
zman clean 0.12.0 # Running simply clean won't remove 0.12.0
```
`zman run <VERSION> [COMMANDS...]`: Run a specific version of Zig with all the following commands

```bash
zman run 0.12.0 build --host-target x86_64-macos
```
## Todo
- Add package manager functionality globally
- Pin a specific version to a folder
