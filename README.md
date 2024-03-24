## Usage
`zigup default [OPTIONS] <VERSION>`: Download and set a Zig version as default
```bash
zigup default latest
zigup default master
zigup default 0.12.0
```
`zigup fetch [OPTIONS] <VERSION>`: Only download a zig version
```bash
zigup fetch latest
zigup fetch master
zigup fetch 0.12.0
```
Options are:
```bash
--install-dir <DIR> # Set the install directory by Default installs to XDG_DATA_HOME/zigup or $HOME/.local/share/zigup
--path-link <PATH> # Set the path to link the binaries to by Default links to /usr/bin
--user # Sudo is used to link to /usr/bin/ by Default. Instead link to $HOME/.local/bin
--no-dropins # Do not create shims like `zig-cc` or `zig-c++` for Zig drop-in tools 
```
`zigup clean [VERSION]`: To clean every version of Zig, except `Default` and `master` or only a particular version.
```bash
zigup clean latest
zigup clean master
zigup clean 0.12.0
```
`zigup list`: List all installed versions of Zig.

`zigup keep <VERSION>`: Prevent a version from being cleaned by `zigup clean`. Can be reverted by cleaning the specific version that is kept.
```bash
zigup keep 0.12.0
```
`zigup run <VERSION> [COMMANDS...]`: Run a specific version of Zig with all the following commands.

```bash
zigup run 0.12.0 build --host-target x86_64-macos
```
## Todo
- Add package manager functionality global and local
- Pin a specific version to a folder