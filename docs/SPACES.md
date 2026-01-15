# Kibana Spaces Management

This guide explains how to manage Kibana Spaces using `kibob`.

## Overview

Kibana Spaces allow you to organize your saved objects (dashboards, visualizations, etc.) into separate namespaces. The `kibob` tool supports managing spaces alongside saved objects, enabling you to:

- Version control space configurations
- Deploy spaces consistently across environments
- Bundle spaces for distribution
- Track space changes in Git

## Getting Started

### 1. Create a Spaces Manifest

Create a `manifest/spaces.yml` file in your project directory with a list of space IDs to manage:

```yaml
spaces:
  - default
  - marketing
  - engineering
```

**Manifest Format:**
- Minimalist YAML structure
- Simple list of space IDs (names)
- Human-readable and easy to edit

### 2. Pull Spaces from Kibana

The `pull` command will automatically pull spaces if a `manifest/spaces.yml` file exists:

```bash
kibob pull ./my-project
```

This creates:
```
my-project/
├── manifest/
│   ├── saved_objects.json
│   └── spaces.yml
├── objects/
│   └── ... (saved objects)
└── spaces/
    ├── default.json
    ├── marketing.json
    └── engineering.json
```

Each space is saved as a pretty-printed JSON file in the `spaces/` directory.

### 3. Push Spaces to Kibana

The `push` command will automatically push spaces if a `manifest/spaces.yml` file exists:

```bash
kibob push ./my-project
```

This will:
- Create new spaces if they don't exist (POST)
- Update existing spaces (PUT)
- Use the space definitions from `spaces/<space_id>.json` files

### 4. Bundle Spaces for Distribution

The `togo` command will automatically bundle spaces to `bundle/spaces.ndjson` if a `manifest/spaces.yml` file exists:

```bash
kibob togo ./my-project
```

This creates:
```
my-project/
└── bundle/
    ├── saved_objects.ndjson  # Saved objects bundle
    └── spaces.ndjson         # Spaces bundle
```

You can easily create a distributable archive:
```bash
cd my-project
zip -r dashboards.zip bundle/
# or
tar -czf dashboards.tar.gz bundle/
```

## Space File Format

Each space is stored as a JSON file with the following structure:

```json
{
  "id": "marketing",
  "name": "Marketing Team",
  "description": "Space for marketing dashboards and analytics",
  "color": "#00BFB3",
  "initials": "MK",
  "disabledFeatures": []
}
```

**Key Fields:**
- `id` (required): Unique identifier for the space
- `name` (required): Display name
- `description` (optional): Human-readable description
- `color` (optional): Hex color code for the space
- `initials` (optional): Short abbreviation (1-2 characters)
- `disabledFeatures` (optional): Array of feature IDs to disable in this space

## Workflows

### Version Control Spaces

1. Create spaces manifest:
   ```yaml
   # manifest/spaces.yml
   spaces:
     - default
     - production
     - staging
   ```

2. Pull spaces from Kibana:
   ```bash
   kibob pull .
   ```

3. Commit to Git:
   ```bash
   git add manifest/spaces.yml spaces/
   git commit -m "Add space configurations"
   ```

### Deploy Spaces to New Environment

1. Clone repository:
   ```bash
   git clone <repo-url>
   cd my-dashboards
   ```

2. Configure Kibana connection:
   ```bash
   export KIBANA_URL=https://kibana.example.com
   export KIBANA_USERNAME=admin
   export KIBANA_PASSWORD=changeme
   ```

3. Push spaces:
   ```bash
   kibob push .
   ```

### Create New Space

1. Manually create space JSON file:
   ```bash
   cat > spaces/data-science.json <<EOF
   {
     "id": "data-science",
     "name": "Data Science",
     "description": "Machine learning and data science workspace",
     "color": "#6092C0",
     "disabledFeatures": []
   }
   EOF
   ```

2. Add to manifest:
   ```yaml
   # manifest/spaces.yml
   spaces:
     - default
     - data-science
   ```

3. Push to Kibana:
   ```bash
   kibob push .
   ```

### Modify Space Configuration

1. Edit the space JSON file:
   ```bash
   vim spaces/marketing.json
   # Update name, description, or other fields
   ```

2. Push changes:
   ```bash
   kibob push .
   ```

3. Commit to Git:
   ```bash
   git add spaces/marketing.json
   git commit -m "Update marketing space description"
   ```

## API Endpoints Used

### Pull (GET)
For each space ID in the manifest, `kibob` calls:
```
GET /api/spaces/space/{space_id}
```

Response is saved to `spaces/{space_id}.json`

### Push (POST/PUT)
For each space file in `spaces/` directory:
- If space exists: `PUT /api/spaces/space/{space_id}`
- If space is new: `POST /api/spaces/space`

### Bundle (NDJSON)
Converts all `spaces/*.json` files to newline-delimited JSON format in `bundle/spaces.ndjson`

## Best Practices

### 1. Minimal Manifest
Keep the manifest simple - just list space IDs:
```yaml
spaces:
  - production
  - staging
  - development
```

### 2. Pretty-Print JSON
Space JSON files are pretty-printed for readability and Git diffs:
```json
{
  "id": "production",
  "name": "Production"
}
```

### 3. Consistent Naming
Use consistent naming conventions for space IDs:
- `kebab-case` for multi-word IDs: `data-science`, `team-alpha`
- No spaces or special characters
- Keep IDs short but descriptive

### 4. Version Control Everything
Commit both manifest and space files:
```bash
git add manifest/spaces.yml spaces/
```

### 5. Document Space Purpose
Use the `description` field to explain the space's purpose:
```json
{
  "id": "production",
  "name": "Production",
  "description": "Production environment - monitored 24/7, contains live customer data"
}
```

### 6. Separate Concerns
- **Spaces**: Organize namespaces and environments
- **Saved Objects**: Store in appropriate space using `KIBANA_SPACE` env var
- **Manifest**: Track which spaces and objects to manage

## Limitations

### Current Implementation
- **Spaces are not deleted**: `kibob` only creates and updates spaces, it does not delete them
- **No space validation**: Space configurations are not validated before push
- **No conflict resolution**: If a space exists with different config, it will be overwritten
- **No space migration**: Moving objects between spaces requires manual work

### Future Enhancements
Potential features for future releases:
- Delete spaces that are removed from manifest
- Validate space configurations before push
- Diff spaces between local and remote
- Migrate objects between spaces
- Space-specific saved objects management

## Troubleshooting

### Space Not Found
```
Failed to fetch space 'my-space': 404 Not Found
```
**Solution**: Remove the space from `manifest/spaces.yml` or create it in Kibana first

### Permission Denied
```
Failed to update space 'production': 403 Forbidden
```
**Solution**: Ensure your Kibana user has space management permissions

### Invalid Space ID
```
Space missing 'id' field
```
**Solution**: Ensure each space JSON file has an `id` field matching the filename

### Manifest Not Found
```
Spaces manifest not found: ./manifest/spaces.yml
```
**Solution**: Create `manifest/spaces.yml` with your space IDs

## Example Project Structure

Complete project with spaces:
```
my-kibana-project/
├── .env                    # Kibana connection settings
├── .gitignore
├── manifest/
│   ├── saved_objects.json  # Saved objects manifest
│   └── spaces.yml          # Spaces manifest
├── objects/                # Saved objects (dashboards, etc.)
│   ├── dashboard/
│   ├── visualization/
│   └── index-pattern/
├── spaces/                 # Space configurations
│   ├── default.json
│   ├── production.json
│   └── staging.json
└── bundle/                 # Bundled files (from togo)
    ├── saved_objects.ndjson
    └── spaces.ndjson
```

## Integration with CI/CD

### GitHub Actions Example
```yaml
name: Deploy Spaces
on:
  push:
    branches: [main]
    paths:
      - 'spaces/**'
      - 'manifest/spaces.yml'

jobs:
  deploy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install kibob
        run: cargo install kibana-object-manager
      
      - name: Deploy Spaces
        env:
          KIBANA_URL: ${{ secrets.KIBANA_URL }}
          KIBANA_USERNAME: ${{ secrets.KIBANA_USERNAME }}
          KIBANA_PASSWORD: ${{ secrets.KIBANA_PASSWORD }}
        run: kibob push .
```

### GitLab CI Example
```yaml
deploy-spaces:
  stage: deploy
  image: rust:latest
  before_script:
    - cargo install kibana-object-manager
  script:
    - kibob push .
  only:
    changes:
      - spaces/**
      - manifest/spaces.yml
  environment:
    name: production
```

## See Also

- [User Guide](USER_GUIDE.md) - Complete command reference
- [Examples](EXAMPLES.md) - Real-world usage scenarios
- [Architecture](ARCHITECTURE.md) - Technical implementation details
