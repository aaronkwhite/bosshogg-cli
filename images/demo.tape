# BossHogg demo tape — generates images/demo.gif
# Prerequisites: brew install vhs
# Run with credentials in env:
#   POSTHOG_CLI_PROJECT_ID=<id> POSTHOG_CLI_HOST=https://us.posthog.com vhs images/demo.tape
#
# Output: images/demo.gif (900x540, ~25s loop)

Output images/demo.gif

Require bosshogg

Set FontSize 15
Set Width 1200
Set Height 675
Set Theme "Dracula"
Set Padding 20
Set Shell "bash"

# Short pause at start
Sleep 1s

# --- Step 1: doctor ---
Type "bosshogg doctor"
Sleep 500ms
Enter
Sleep 2500ms

# --- Step 2: flag list ---
Type "bh flag list --active --json | jq '.results[0:2]'"
Sleep 500ms
Enter
Sleep 3s

# --- Step 3: HogQL query ---
Type 'bh query run "SELECT event, count() FROM events GROUP BY event ORDER BY count() DESC LIMIT 5"'
Sleep 500ms
Enter
Sleep 3500ms

# Hold on final frame
Sleep 2s
