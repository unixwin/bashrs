# Windows-Native Installation Layout

This is the installation contract for WinuxCmd, WPM, Winuxsh, and Rubash.

## Installation Root

The selected WinuxCmd installation directory is the real filesystem root of
the Unix-style command tree. For example:

```text
C:/Users/Administrator/AppData/Local/Programs/Winuxsh/winuxcmd/
  usr/bin/
  bin/
  usr/local/bin/
  etc/
  var/
  tmp/
  dev/
  .wpm/
```

These are ordinary Windows directories. There is no second shell root and no
directory overlay assembled by individual commands.

## Executable Placement

- `usr/bin/` is the canonical directory for WinuxCmd and WPM executables.
- `bin/` and `usr/local/bin/` are real package target directories.
- A package target explicitly under `bin/`, `usr/bin/`, or `usr/local/bin/`
  is installed in that exact directory.
- A legacy package target with only a filename is normalized to `usr/bin/`
  for new installs.
- `.wpm/` remains private WPM state and is never part of a command directory.

## Compatibility

Existing flat files next to the old `winuxcmd.exe` remain discoverable during
the compatibility period. New installers and new WPM installs use the real
directory layout. Migration may create a hard link, a Windows symbolic link,
or a copied file when the stronger form is unavailable; it must not delete an
existing user file merely to migrate it.

## Runtime Ownership

- The installer creates the directory tree and places `winuxcmd.exe` and
  `wpm.exe` in `usr/bin/`.
- WPM owns package payload placement and synchronization inside the tree.
- Winuxsh selects the installation root and puts the real bin directories on
  `PATH`.
- Rubash consumes the resulting native Windows paths for command execution,
  redirection, globbing, completion, and external processes.
- WinuxCmd coreutils do not inspect Winuxsh variables and do not implement
  provider or overlay behavior.

## Special Paths

- `~` is the Windows `USERPROFILE` directory.
- `tmp/`, `etc/`, `var/`, and `dev/` are real directories under the selected
  installation root.
- `/dev/null` is the one supported Windows device endpoint and is opened as
  `NUL`; other `/dev` entries are not implied by this contract.

