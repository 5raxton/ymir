#!/usr/bin/env fish
# ymir installer (fish entry point).
#
# This is a thin wrapper around the bash installer, which handles distro
# detection, runtime dependency install, download of the latest pre-built
# binary, install of the session .desktop + default config, and updating.
# Keeping the logic in one place avoids drift between the bash and fish
# versions.

set SCRIPT_DIR (dirname (status --current-filename))
set BASH_INSTALLER "$SCRIPT_DIR/install.sh"

if not test -f "$BASH_INSTALLER"
    echo "error: $BASH_INSTALLER is missing." >&2
    exit 1
end

# Ensure the bash script is runnable; use bash explicitly to avoid shebang/env
# issues if it was checked out without the executable bit.
bash "$BASH_INSTALLER" $argv
exit $status
