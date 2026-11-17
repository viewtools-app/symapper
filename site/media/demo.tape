# Re-record from the repository root: vhs site/media/demo.tape
# Needs: vhs, a symapper binary on PATH (the Env line below expects it in /tmp/demobin),
# and a git repo at /tmp/symapper-demo-repo: a copy of this repository without tests/, npm/,
# .github/, *.toml, *.md and LICENSE, committed, so the map is not diluted by non-source files.
Output site/media/demo.webm
Output site/media/demo.mp4
Output site/media/demo.gif
Set Shell "bash"
Set FontFamily "Menlo"
Set FontSize 16
Set Width 1000
Set Height 700
Set Padding 22
Set LineHeight 1.25
Set Framerate 10
Set TypingSpeed 45ms
Set Theme { "name": "symapper", "black": "#2a2a28", "red": "#a02e08", "green": "#2f6b3a", "yellow": "#8a5a00", "blue": "#2b4f8a", "magenta": "#7a3a7a", "cyan": "#1f6b6b", "white": "#ebebe6", "brightBlack": "#61615a", "brightRed": "#a02e08", "brightGreen": "#2f6b3a", "brightYellow": "#8a5a00", "brightBlue": "#2b4f8a", "brightMagenta": "#7a3a7a", "brightCyan": "#1f6b6b", "brightWhite": "#fbfbf9", "background": "#fbfbf9", "foreground": "#2a2a28", "selection": "#ebebe6", "cursor": "#a02e08" }
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
Screenshot site/media/poster.png
Sleep 3500ms
