# Appended to ~/.bashrc and ~/.profile.
# .bashrc returns early for non-interactive shells, so the .profile copy is
# what stops scripts and hooks missing cargo and falling through to whatever
# else is on PATH.
. "$HOME/.cargo/env"
