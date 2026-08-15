# Windows-Native Path Root

On Windows, Rubash uses the real WinuxCmd installation directory as the shell
root. This is only path spelling support for `/bin`, `/usr/bin`, and similar
Unix-style names; it is not a POSIX runtime, MSYS, WSL, Cygwin mount, or a
directory overlay.

The installer and Winuxsh select one installation root and create ordinary
Windows directories below it:

```text
<WinuxCmd installation root>/
  usr/bin/
  bin/
  usr/local/bin/
  etc/
  var/
  tmp/
  dev/
  .wpm/
```

`usr/bin` is canonical for `winuxcmd.exe`, `wpm.exe`, and filename-only WPM
targets. Explicit WPM targets under `bin`, `usr/bin`, or `usr/local/bin` stay
in that real directory. `.wpm` is private state and is not added to `PATH` or
directory listings.

Rubash maps paths lexically below this root:

```text
/             -> <root>
/usr/bin/tool -> <root>/usr/bin/tool
/bin/tool    -> <root>/bin/tool
/etc/config  -> <root>/etc/config
/tmp/file    -> <root>/tmp/file
```

Command lookup and native child `PATH` use these real directories directly.
Rubash does not merge files from a separate WinuxCmd provider directory, and
coreutils do not inspect Winuxsh variables. Existing flat installations remain
usable when their directory is explicitly placed on `PATH`; new installers do
not create that layout.

Winuxsh owns root selection and sets `WINUXSH_ROOT`; embedding callers may use
`RUBASH_ROOT` or `Executor::set_shell_root`. `WINUXCMD_PATH` selects the one
explicit dispatcher executable, while `WINUXCMD_HOME` records its installation
root. A dispatcher is only a fallback for commands absent from the real tree.

On Windows, `~` is `USERPROFILE`, the same directory used by PowerShell. The
only device spelling currently supported is `/dev/null`, which maps to the
native `NUL` endpoint. Other `/dev` entries do not become ordinary files.
