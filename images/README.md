# images/

Visual assets used in the README and external surfaces.

| File | Dimensions | Purpose |
|---|---|---|
| `bosshogg-repo-banner.png` | 1600×480 | README hero (top of page) |
| `demo.gif` | 900×540 | README quickstart demo — regenerate via `vhs demo.tape` |
| `demo.tape` | — | VHS script source for demo.gif |
| `social-card.png` | 1280×640 | GitHub repo social preview (Settings → General → Social preview) |
| `hero-square.png` | 1024×1024 | Product Hunt thumbnail (rendered at 240×240); favicon source |
| `comparison-mcp.png` | 1270×760 | PH gallery slide 2; LinkedIn post 2; blog cover |
| `agent-flow.png` | 1270×760 | PH gallery slide 3; LinkedIn post 3 |
| `ph-gallery-1-hero.png` | 1270×760 | PH gallery slot 1 |
| `ph-gallery-2-comparison.png` | 1270×760 | PH gallery slot 2 |
| `ph-gallery-3-agent.png` | 1270×760 | PH gallery slot 3 |
| `ph-gallery-4-coverage.png` | 1270×760 | PH gallery slot 4 |
| `ph-gallery-5-safety.png` | 1270×760 | PH gallery slot 5 |

## Regenerating demo.gif

```bash
# Install vhs if needed
brew install vhs

# Run with your credentials
POSTHOG_CLI_PROJECT_ID=<your-project-id> \
POSTHOG_CLI_HOST=https://us.posthog.com \
vhs demo.tape
```

## Optimizing PNGs

```bash
./scripts/optimize-images.sh
```
