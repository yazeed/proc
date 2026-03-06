# proc demo — recorded inside Docker for a clean environment

Output demo.gif

Set Shell bash
Set FontSize 15
Set Width 1000
Set Height 550
Set Padding 20
Set Theme "Catppuccin Mocha"
Set TypingSpeed 50ms
Set PlaybackSpeed 1

# --- What's on port 3000? ---
Type "proc on :3000"
Enter
Sleep 2.5s

# --- Reverse lookup: what ports is node using? ---
Type "proc on node"
Enter
Sleep 2.5s

# --- Filter by name + directory ---
Type "proc by python3 --in /tmp/myapp"
Enter
Sleep 2.5s

# --- List top processes ---
Type "proc list --sort cpu --limit 8"
Enter
Sleep 2.5s

# --- All listening ports ---
Type "proc ports"
Enter
Sleep 2.5s

# --- Multi-target kill with dry-run ---
Type "proc kill :3000,:8080 --dry-run"
Enter
Sleep 2.5s

# --- Detailed info ---
Type "proc info :3000,:8080"
Enter
Sleep 3s

# --- JSON output for scripting ---
Type "proc on :3000 --json"
Enter
Sleep 2.5s

# --- Done ---
Sleep 1.5s
