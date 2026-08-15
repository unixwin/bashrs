# Windows Logical Root Contract

Rubash uses a logical Unix-shaped path namespace on Windows. This namespace
is a path adapter, not a POSIX runtime and not an MSYS, WSL, or Cygwin mount.
Windows filesystem APIs, process creation, handles, pipes, and devices remain
the host implementation.

## Ownership

| Area | Owner |
| --- | --- |
| Bash path spelling, `cd`, `source`, redirection targets, and command lookup | Rubash |
| Backing files and directories | Windows filesystem, below the configured root |
| External command implementation and dispatch | WinuxCmd |
| Root creation, session configuration, terminal, and temporary directory policy | Winuxsh |
| Shell file descriptors and `/dev/fd` semantics | Rubash fd table, using WinuxCmd handle capabilities when needed |

The selected WinuxCmd installation is a separate native command provider. Its
directory contains `winuxcmd.exe`, `wpm.exe`, command links such as `cat.exe`
and `ls.exe`, and WPM-installed native command files such as `jq.exe`. It is
not copied below the logical root.

On Windows, `~` is the same user directory selected by PowerShell: Winuxsh
uses `USERPROFILE` as the authoritative value. `HOME` is normalized for child
process compatibility, but it is not a second home namespace.

Winuxsh should create or select a private backing directory and expose it as
`WINUXSH_ROOT` before constructing Rubash. A typical layout is:

```text
<winuxsh-data>/root/
  bin/
  usr/bin/
  usr/local/bin/
  etc/
  var/
  tmp/
```

Rubash also accepts `RUBASH_ROOT` for embedding scenarios and the internal
`__RUBASH_SHELL_ROOT` value set by `Executor::set_shell_root`. The first
non-empty value in that order is used.

## Path resolution

With a root configured, absolute logical paths are resolved lexically below
that root:

```text
/             -> <root>
/bin/tool     -> <root>/bin/tool
/usr/bin/tool -> <root>/usr/bin/tool
/etc/config   -> <root>/etc/config
/tmp/file     -> <root>/tmp/file
```

`.` and `..` are normalized in the logical namespace, so `/../etc` resolves
to `<root>/etc`. This lexical rule is not a security boundary: a backing
symlink can still point outside the root. A future sandboxed mode must enforce
that separately with Windows handle-based traversal.

Windows paths such as `C:/work/file` and the explicit display spelling
`/c/work/file` remain host paths and are not captured by the logical root.
There is no lookup of MSYS, Git Bash, WSL, or another compatible shell.

When a root is configured, `command -p` uses the logical standard path:
`/usr/local/bin:/usr/bin:/bin`. The normal `PATH` remains caller-controlled.

On Windows, the three logical command directories have a read/execute
provider overlay. Rubash checks the backing root first, then the selected
WinuxCmd installation directory. This makes `/usr/bin/ls`, `/bin/cat`, and
commands added later by WPM resolve to installed native command files without
creating duplicate files in the root. WPM's `.wpm/cache`, `.wpm/staging`,
`.wpm/backup`, and index files remain private installation state and are never
part of `/`, `/usr/bin`, or shell glob results.

The backing directories are real directories. `/etc`, `/var`, and `/tmp` are
ordinary Windows directories below the logical root, while `/dev` is a
capability namespace and is not created as a normal directory.

The provider lookup accepts both the current flat installation layout and a
future provider layout containing `bin/`, `usr/bin/`, or `usr/local/bin/`.
The logical directory selects its matching provider subdirectory first, then
falls back to the flat layout. This is lookup policy, not filesystem
synchronization.

WPM package mappings are relative to the WinuxCmd installation root. The
current official index installs command files at the provider root, so a
downloaded `jq.exe` becomes `<winuxcmd>/jq.exe`; a future mapping such as
`usr/bin/tool.exe` remains below the provider's matching logical directory.
Package installation first tries a Windows hardlink and falls back to a copy
when that is necessary. `wpm links rebuild` is different: it rebuilds the
core command links with hardlinks and reports failures rather than silently
switching to symlinks. Neither operation writes `.wpm` into the logical root.

For native child processes, Rubash materializes each logical command PATH
entry as two Windows entries: the corresponding root backing directory and
the selected provider directory. The shell session selects one exact
`winuxcmd.exe` through `WINUXCMD_PATH`; stale WinuxCmd provider directories
are removed from the session PATH so command links and dispatcher cannot be
mixed across installations.

## WinuxCmd dispatch

Rubash first checks the logical root for an executable. If the command is not
file-backed, WinuxCmd may expose one native dispatcher executable. Its path is
configured explicitly with `WINUXCMD_PATH` (or `WINUXCMD`), or can be set with
`Executor::set_winuxcmd_path`. Rubash never discovers `winuxcmd.exe` from
`PATH`; Winuxsh selects one version for the session and injects that path.
The dispatcher is queried before a command is reported as not found. For
`/usr/bin/head`, it receives the command name `head`, followed by the
finalized shell arguments.

The dispatcher is an external-command backend. It must not implement Rubash
builtins such as `cd`, `export`, `set`, `read`, `jobs`, or `trap`.

## Devices

`/dev` is a capability namespace, not a directory that should be copied into
the root. The currently supported device is `/dev/null`, mapped to Windows
`NUL` for I/O and exposed to `test` as a virtual character device. `/dev/fd/N`,
`/dev/stdin`, `/dev/stdout`, `/dev/stderr`, and `/dev/tty` require explicit fd
table or terminal capabilities. On Windows they are rejected by the path
adapter and must not fall through to ordinary root files until those
capabilities are wired.

This split keeps the Windows-native implementation small and explicit while
still giving scripts a stable logical `/`, `/bin`, `/usr/bin`, `/etc`, `/tmp`,
and `/dev/null` contract.
