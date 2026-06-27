# Kibana Object Manager - Quick Reference

Fast reference for `kibob` commands and options.

## Commands

| Command | Description | Example |
|---------|-------------|---------|
| `kibob auth` | Test connection to Kibana | `kibob auth` |
| `kibob init` | Initialize project from export | `kibob init export.ndjson ./dashboards` |
| `kibob pull` | Fetch objects from Kibana | `kibob pull .` |
| `kibob push` | Upload objects to Kibana | `kibob push . --managed true` |
| `kibob add` | Add objects to manifest | `kibob add . --objects "dashboard=abc123"` |
| `kibob togo` | Bundle objects to NDJSON | `kibob togo .` |
| `kibob migrate` | Migrate legacy manifest | `kibob migrate .` |

## Environment Variables

| Variable | Required | Description | Example |
|----------|----------|-------------|---------|
| `KIBANA_URL` | Yes | Kibana base URL | `http://localhost:5601` |
| `KIBANA_USERNAME` | No* | Basic auth username | `elastic` |
| `KIBANA_PASSWORD` | No* | Basic auth password | `changeme` |
| `KIBANA_APIKEY` | No* | API key (conflicts with user/pass) | `base64encodedkey` |

*Must provide either USERNAME/PASSWORD or APIKEY

## Global Flags

| Flag | Description | Example |
|------|-------------|---------|
| `--env <file>` | Load environment from file | `kibob --env .env.prod pull .` |
| `--debug` | Enable verbose logging | `kibob --debug push .` |
| `--help` | Show help for command | `kibob push --help` |
| `--version` | Show version information | `kibob --version` |

## Common Patterns

### Initial Setup
```bash
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
kibob auth
kibob init export.ndjson ./dashboards
```

### Daily Development Workflow
```bash
# Pull changes from Kibana
kibob pull .
git diff
git commit -am "Update dashboard"

# Push changes to Kibana
vim objects/dashboard/my-dash.json
kibob push .
```

### Multi-Environment Deployment
```bash
# Development
kibob --env .env.dev pull .

# Staging
kibob --env .env.staging push . --managed true

# Production
kibob --env .env.prod push . --managed true
```

### Team Collaboration
```bash
# Pull teammate's changes
git pull origin main
kibob push .  # Deploy to your Kibana

# Share your changes
kibob pull .
git add . && git commit -m "New dashboard"
git push origin main
```

## Project Structure

```
my-dashboards/
├── .env                           # Environment config (gitignored)
├── manifest/
│   └── saved_objects.json        # Tracks managed objects
└── objects/
    ├── dashboard/
    │   ├── abc-123.json          # Dashboard objects
    │   └── xyz-789.json
    ├── visualization/
    │   └── def-456.json          # Visualization objects
    ├── index-pattern/
    │   └── logs-*.json           # Index patterns
    └── search/
        └── saved-search.json     # Saved searches
```

## Manifest Format

```json
{
  "version": "1.0",
  "objects": [
    {
      "type": "dashboard",
      "id": "abc-123",
      "attributes": {
        "title": "My Dashboard"
      }
    }
  ]
}
```

## Object Types

Common Kibana object types:
- `dashboard` - Kibana dashboards
- `visualization` - Visualizations
- `lens` - Lens visualizations
- `index-pattern` - Index patterns
- `search` - Saved searches
- `map` - Maps
- `canvas-workpad` - Canvas workpads

## Authentication Methods

### Basic Auth
```bash
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
```

### API Key
```bash
export KIBANA_APIKEY=your_base64_encoded_key
```

**Create API Key in Kibana:**
1. Stack Management → API Keys
2. Create API key
3. Copy encoded value to `KIBANA_APIKEY`

## Managed vs. Unmanaged Objects

| Flag | Effect | Use Case |
|------|--------|----------|
| `--managed true` | Read-only in Kibana UI | Production deployments |
| `--managed false` | Editable in Kibana UI | Development, testing |

```bash
# Production: prevent manual edits
kibob push . --managed true

# Development: allow quick iterations
kibob push . --managed false
```

## Troubleshooting Quick Fixes

### Connection refused
```bash
curl $KIBANA_URL/api/status  # Test Kibana is running
```

### 401 Unauthorized
```bash
kibob auth  # Verify credentials
env | grep KIBANA  # Check environment variables
```

### Manifest not found
```bash
ls -la manifest/  # Check directory structure
kibob migrate .   # Migrate legacy format
```

### Object not found
```bash
# Remove missing object from manifest
vim manifest/saved_objects.json
```

### Debug mode
```bash
kibob --debug pull .  # Verbose logging
```

## Quick Start Checklist

- [ ] Install kibob: `cargo install kibana-object-manager`
- [ ] Export dashboards from Kibana UI
- [ ] Set environment variables (URL, credentials)
- [ ] Test connection: `kibob auth`
- [ ] Initialize project: `kibob init export.ndjson ./dashboards`
- [ ] Initialize Git: `cd dashboards && git init`
- [ ] First commit: `git add . && git commit -m "Initial import"`
- [ ] Test pull: `kibob pull .`
- [ ] Test push: `kibob push .`

## Common Object ID Formats

```bash
# Add by type and ID
kibob add . --objects "dashboard=abc-123-def-456"
kibob add . --objects "visualization=xyz-789"
kibob add . --objects "index-pattern=logs-*"

# Multiple objects
kibob add . --objects "dashboard=id1,visualization=id2,lens=id3"
```

## Exit Codes

| Code | Meaning |
|------|---------|
| 0 | Success |
| 1 | General error |
| 101 | Authentication failed |
| 102 | Connection failed |
| 103 | Manifest not found |
| 104 | Object not found |

## File Patterns to .gitignore

```gitignore
# Kibana Object Manager
.env*
*.ndjson
manifest.json.bak

# Build artifacts (if building from source)
/target
Cargo.lock
```

## Useful Aliases

Add to your `~/.bashrc` or `~/.zshrc`:

```bash
# Kibana Object Manager aliases
alias kauth='kibob auth'
alias kpull='kibob pull .'
alias kpush='kibob push . --managed true'
alias kdev='kibob --env .env.dev'
alias kprod='kibob --env .env.prod'
alias kdiff='kibob pull . && git diff objects/'
```

## Resources

- **Documentation:** https://github.com/VimCommando/kibana-object-manager/tree/main/docs
- **Issues:** https://github.com/VimCommando/kibana-object-manager/issues
- **Discussions:** https://github.com/VimCommando/kibana-object-manager/discussions

## Version Information

Check your version:
```bash
kibob --version
```

Update to latest:
```bash
cargo install --force kibana-object-manager
```

---

**Need more details?** See the full [User Guide](USER_GUIDE.md).
