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

Create a `spaces.yml` file in your project directory with a list of spaces to manage:

```yaml
spaces:
  - id: default
    name: Default
  - id: marketing
    name: Marketing Team
  - id: engineering
    name: Engineering
```

**Manifest Format:**
- YAML structure with space entries
- Each entry requires `id` and `name` fields
- The `id` is the space identifier used in URLs
- The `name` is the display name shown in Kibana UI
- Human-readable and easy to edit

### 2. Pull Spaces from Kibana

The `pull` command will automatically pull spaces if a `spaces.yml` file exists:

```bash
kibob pull ./my-project
```

This creates:
```
my-project/
├── spaces.yml               # Managed spaces list
├── default/
│   ├── space.json           # Space definition
│   ├── manifest/            # Per-space manifests
│   │   └── saved_objects.json
│   └── objects/
│       └── ... (saved objects)
├── marketing/
│   ├── space.json
│   ├── manifest/
│   │   └── saved_objects.json
│   └── objects/
│       └── ...
└── engineering/
    ├── space.json
    ├── manifest/
    │   └── saved_objects.json
    └── objects/
        └── ...
```

Each space's definition is saved in its own directory as `{space_id}/space.json`.

### 3. Push Spaces to Kibana

The `push` command will automatically push spaces if a `spaces.yml` file exists:

```bash
kibob push ./my-project
```

This will:
- Create new spaces if they don't exist (POST)
- Update existing spaces (PUT)
- Use the space definitions from `{space_id}/space.json` files

### 4. Bundle Spaces for Distribution

The `togo` command will automatically bundle spaces to `bundle/spaces.ndjson` if a `spaces.yml` file exists:

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
   # spaces.yml
   spaces:
     - id: default
       name: Default
     - id: production
       name: Production
     - id: staging
       name: Staging
   ```

2. Pull spaces from Kibana:
   ```bash
   kibob pull .
   ```

3. Commit to Git:
   ```bash
   git add spaces.yml default/space.json production/space.json staging/space.json
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
   mkdir -p data-science
   cat > data-science/space.json <<EOF
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
   # spaces.yml
   spaces:
     - id: default
       name: Default
     - id: data-science
       name: Data Science
   ```

3. Push to Kibana:
   ```bash
   kibob push .
   ```

### Modify Space Configuration

1. Edit the space JSON file:
   ```bash
   vim marketing/space.json
   # Update name, description, or other fields
   ```

2. Push changes:
   ```bash
   kibob push .
   ```

3. Commit to Git:
   ```bash
   git add marketing/space.json
   git commit -m "Update marketing space description"
   ```

## API Endpoints Used

### Pull (GET)
For each space ID in the manifest, `kibob` calls:
```
GET /api/spaces/space/{space_id}
```

Response is saved to `{space_id}/space.json`

### Push (POST/PUT)
For each space file in space directories:
- If space exists: `PUT /api/spaces/space/{space_id}`
- If space is new: `POST /api/spaces/space`

### Bundle (NDJSON)
Converts all `{space_id}/space.json` files to newline-delimited JSON format in `bundle/spaces.ndjson`

## Best Practices

### 1. Structured Manifest
Keep the manifest clear with id and name for each space:
```yaml
spaces:
  - id: production
    name: Production
  - id: staging
    name: Staging
  - id: development
    name: Development
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
git add spaces.yml */space.json
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
**Solution**: Remove the space from `spaces.yml` or create it in Kibana first

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
Spaces manifest not found: ./spaces.yml
```
**Solution**: Create `spaces.yml` with your space IDs

## Example Project Structure

Complete project with spaces:
```
my-kibana-project/
├── .env                    # Kibana connection settings
├── .gitignore
├── spaces.yml              # Spaces manifest
├── default/                # Self-contained space directory
│   ├── space.json          # Space definition
│   ├── manifest/           # Per-space manifests
│   │   ├── saved_objects.json
│   │   ├── workflows.yml
│   │   ├── agents.yml
│   │   └── tools.yml
│   ├── objects/            # Saved objects
│   │   ├── dashboard/
│   │   ├── visualization/
│   │   └── index-pattern/
│   ├── workflows/          # Workflow definitions
│   ├── agents/             # Agent definitions
│   └── tools/              # Tool definitions
├── production/             # Another self-contained space
│   ├── space.json
│   ├── manifest/
│   │   └── saved_objects.json
│   └── objects/
└── bundle/                 # Bundled files (from togo)
    ├── default/
    │   ├── saved_objects.ndjson
    │   └── workflows.ndjson
    ├── production/
    │   └── saved_objects.ndjson
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
      - '*/space.json'
      - 'spaces.yml'

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
      - '*/space.json'
      - spaces.yml
  environment:
    name: production
```

## See Also

- [User Guide](USER_GUIDE.md) - Complete command reference
- [Examples](EXAMPLES.md) - Real-world usage scenarios
- [Architecture](ARCHITECTURE.md) - Technical implementation details
