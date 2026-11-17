# Dark variant of demo.tape: only the outputs and the Theme differ. Re-record from the repository root: vhs site/media/demo-dark.tape
# Needs: vhs, a symapper binary on PATH (the Env line below expects it in /tmp/demobin),
# and a git repo at /tmp/symapper-demo-repo: a copy of this repository without tests/, npm/,
# .github/, *.toml, *.md and LICENSE, committed, so the map is not diluted by non-source files.
Output site/media/demo-dark.webm
Output site/media/demo-dark.mp4
Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 16
Set Width 1000
Set Height 700
Set Padding 22
Set LineHeight 1.25
Set Framerate 10
Set TypingSpeed 45ms
Set Theme { "name": "symapper-dark", "black": "#1d1e1a", "red": "#ffab80", "green": "#9fd8a8", "yellow": "#e6c070", "blue": "#9dbdf2", "magenta": "#d9a6d9", "cyan": "#8fd4d4", "white": "#d8d8cf", "brightBlack": "#b7b7ab", "brightRed": "#ffab80", "brightGreen": "#9fd8a8", "brightYellow": "#e6c070", "brightBlue": "#9dbdf2", "brightMagenta": "#d9a6d9", "brightCyan": "#8fd4d4", "brightWhite": "#f2f2eb", "background": "#1d1e1a", "foreground": "#f2f2eb", "selection": "#3a3b34", "cursor": "#ffab80" }
Env PS1 "$ "
Env PATH "/tmp/demobin:/opt/homebrew/bin:/usr/bin:/bin"
Env SYMAPPER_CACHE_DIR "/tmp/demo-cache"

Hide
Type "mkdir -p /tmp/demo-cache && cd /tmp/symapper-demo-repo && clear"
Enter
Sleep 500ms
Show

Sleep 800ms
Type "# Task: make the cache directory configurable"
Sleep 1s
Enter
Sleep 600ms
Type "symapper . --focus cache --tokens 100"
Sleep 800ms
Enter
Sleep 5s
Type "# Then verify with grep"
Enter
Sleep 400ms
Type "grep -n SYMAPPER_CACHE_DIR src/cli.rs | head -3"
Sleep 600ms
Enter
Sleep 1s
Screenshot site/media/poster-dark.png
Sleep 3500ms
