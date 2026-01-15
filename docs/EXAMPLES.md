# Kibana Object Manager - Real-World Examples

Practical examples and recipes for common use cases.

## Table of Contents

- [For Kibana Admins](#for-kibana-admins)
- [For Developers](#for-developers)
- [For DevOps Engineers](#for-devops-engineers)
- [Advanced Scenarios](#advanced-scenarios)

---

## For Kibana Admins

### Example 1: Weekly Dashboard Backup

**Scenario:** Automatically back up production dashboards every week.

**Setup:**

Create a backup script `backup-dashboards.sh`:
```bash
#!/bin/bash
set -e

# Configuration
BACKUP_DIR="$HOME/kibana-backups"
PROJECT_NAME="production-dashboards"
DATE=$(date +%Y-%m-%d)

# Load production credentials
export KIBANA_URL="https://prod-kibana.example.com"
export KIBANA_APIKEY="your_prod_api_key"

# Create dated backup directory
BACKUP_PATH="$BACKUP_DIR/$PROJECT_NAME-$DATE"
mkdir -p "$BACKUP_PATH"

# Pull from Kibana
echo "Backing up dashboards from $KIBANA_URL..."
kibob pull "$BACKUP_PATH"

# Create archive
cd "$BACKUP_DIR"
tar -czf "$PROJECT_NAME-$DATE.tar.gz" "$PROJECT_NAME-$DATE"
echo "Backup saved: $PROJECT_NAME-$DATE.tar.gz"

# Keep only last 30 days
find "$BACKUP_DIR" -name "$PROJECT_NAME-*.tar.gz" -mtime +30 -delete
```

Add to crontab:
```bash
# Run every Sunday at 2 AM
0 2 * * 0 /path/to/backup-dashboards.sh >> /var/log/kibana-backup.log 2>&1
```

---

### Example 2: Restore Deleted Dashboard

**Scenario:** A dashboard was accidentally deleted in production. Restore it from Git.

```bash
# 1. Clone your dashboard repository
git clone https://github.com/yourorg/kibana-dashboards.git
cd kibana-dashboards

# 2. Find the dashboard in your objects
ls -la objects/dashboard/
# Let's say the deleted dashboard is: sales-overview-abc123.json

# 3. Set production credentials
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_api_key

# 4. Verify connection
kibob auth

# 5. Push the dashboard (it will be recreated)
kibob push . --managed true

# 6. Verify restoration
kibob pull ./verify-restore
diff -r objects/ verify-restore/objects/
```

---

### Example 3: Clone Dashboards to New Space

**Scenario:** Copy all dashboards from default space to a new team space.

```bash
# 1. Pull from default space
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
export KIBANA_SPACE=default

kibob pull ./dashboards

# 2. Push to new space
export KIBANA_SPACE=team-alpha
kibob push ./dashboards --managed false  # Unmanaged so team can customize

echo "Dashboards cloned to team-alpha space"
```

---

### Example 4: Migrate Dashboards Between Clusters

**Scenario:** Move dashboards from old Kibana cluster to new cluster.

```bash
# 1. Export from old cluster
export KIBANA_URL=https://old-kibana.example.com
export KIBANA_USERNAME=admin
export KIBANA_PASSWORD=old_password

kibob pull ./migration-dashboards

# 2. Create archive for audit trail
tar -czf migration-$(date +%Y%m%d).tar.gz migration-dashboards/

# 3. Import to new cluster
export KIBANA_URL=https://new-kibana.example.com
export KIBANA_APIKEY=new_cluster_api_key

# Test connection
kibob auth

# Import as unmanaged for testing
kibob push ./migration-dashboards --managed false

# After verification, push as managed
kibob push ./migration-dashboards --managed true
```

---

## For Developers

### Example 5: Version Control Dashboards with Application Code

**Scenario:** Store observability dashboards in your application's Git repository.

**Project Structure:**
```
my-application/
├── src/
│   └── ... (application code)
├── tests/
│   └── ...
├── dashboards/          # Kibana dashboards
│   ├── .env.example
│   ├── manifest/
│   │   └── saved_objects.json
│   └── objects/
│       ├── dashboard/
│       │   ├── app-overview.json
│       │   └── error-tracking.json
│       └── index-pattern/
│           └── app-logs-*.json
└── README.md
```

**Workflow:**

```bash
# Developer setup
cd my-application/dashboards
cp .env.example .env
# Edit .env with your dev Kibana credentials

# Deploy dashboards to dev Kibana
kibob push . --managed false

# Make changes to dashboard in Kibana
# Pull changes back
kibob pull .
git diff objects/

# Commit with application code
git add .
git commit -m "feat: Add error rate tracking to dashboard"
git push origin feature/error-tracking
```

**CI/CD Integration (.github/workflows/deploy.yml):**
```yaml
name: Deploy Application

on:
  push:
    branches: [main]

jobs:
  deploy-dashboards:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install kibob
        run: cargo install kibana-object-manager
      
      - name: Deploy Dashboards
        env:
          KIBANA_URL: ${{ secrets.KIBANA_URL }}
          KIBANA_APIKEY: ${{ secrets.KIBANA_APIKEY }}
        run: |
          cd dashboards
          kibob auth
          kibob push . --managed true
  
  deploy-application:
    needs: deploy-dashboards
    runs-on: ubuntu-latest
    steps:
      - name: Deploy app
        run: |
          # ... deploy application
```

---

### Example 6: Feature Branch Dashboards

**Scenario:** Test dashboard changes in a feature branch environment.

```bash
# Create feature branch
git checkout -b feature/new-metrics-dashboard

# Create new dashboard in Kibana
# Add it to manifest
kibob add ./dashboards --objects "dashboard=new-metrics-abc123"

# Commit changes
git add dashboards/
git commit -m "Add new metrics dashboard"
git push origin feature/new-metrics-dashboard

# Deploy to feature environment
export KIBANA_URL=https://feature-kibana.dev.example.com
export KIBANA_APIKEY=feature_env_key
kibob push ./dashboards

# After code review and merge
git checkout main
git merge feature/new-metrics-dashboard

# Deploy to production
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_key
kibob push ./dashboards --managed true
```

---

### Example 7: Local Development with Docker Compose

**Scenario:** Run local Kibana for dashboard development.

**docker-compose.yml:**
```yaml
version: '3.8'
services:
  elasticsearch:
    image: docker.elastic.co/elasticsearch/elasticsearch:8.11.0
    environment:
      - discovery.type=single-node
      - xpack.security.enabled=false
    ports:
      - "9200:9200"
  
  kibana:
    image: docker.elastic.co/kibana/kibana:8.11.0
    environment:
      - ELASTICSEARCH_HOSTS=http://elasticsearch:9200
    ports:
      - "5601:5601"
    depends_on:
      - elasticsearch
```

**Development workflow:**
```bash
# Start local stack
docker-compose up -d

# Wait for Kibana to be ready
until curl -s http://localhost:5601/api/status | grep -q "available"; do
  echo "Waiting for Kibana..."
  sleep 5
done

# Deploy dashboards
export KIBANA_URL=http://localhost:5601
export KIBANA_USERNAME=elastic
export KIBANA_PASSWORD=changeme
kibob push ./dashboards --managed false

# Develop and iterate
# ... make changes in Kibana UI ...
kibob pull ./dashboards
git diff

# Clean up
docker-compose down -v
```

---

## For DevOps Engineers

### Example 8: Multi-Environment CI/CD Pipeline

**Scenario:** Automated dashboard deployment across dev → staging → prod.

**Jenkins Pipeline:**
```groovy
pipeline {
    agent any
    
    environment {
        DASHBOARDS_DIR = './dashboards'
    }
    
    stages {
        stage('Install kibob') {
            steps {
                sh 'cargo install kibana-object-manager'
            }
        }
        
        stage('Deploy to Dev') {
            environment {
                KIBANA_URL = credentials('kibana-dev-url')
                KIBANA_APIKEY = credentials('kibana-dev-apikey')
            }
            steps {
                sh '''
                    cd $DASHBOARDS_DIR
                    kibob auth
                    kibob push . --managed true
                '''
            }
        }
        
        stage('Test Dev Deployment') {
            environment {
                KIBANA_URL = credentials('kibana-dev-url')
                KIBANA_APIKEY = credentials('kibana-dev-apikey')
            }
            steps {
                sh '''
                    cd $DASHBOARDS_DIR
                    kibob pull ./verify
                    diff -r objects/ verify/objects/
                '''
            }
        }
        
        stage('Deploy to Staging') {
            when {
                branch 'main'
            }
            environment {
                KIBANA_URL = credentials('kibana-staging-url')
                KIBANA_APIKEY = credentials('kibana-staging-apikey')
            }
            steps {
                sh '''
                    cd $DASHBOARDS_DIR
                    kibob auth
                    kibob push . --managed true
                '''
            }
        }
        
        stage('Approve Production') {
            when {
                branch 'main'
            }
            steps {
                input message: 'Deploy to Production?', ok: 'Deploy'
            }
        }
        
        stage('Deploy to Production') {
            when {
                branch 'main'
            }
            environment {
                KIBANA_URL = credentials('kibana-prod-url')
                KIBANA_APIKEY = credentials('kibana-prod-apikey')
            }
            steps {
                sh '''
                    cd $DASHBOARDS_DIR
                    kibob auth
                    kibob push . --managed true
                '''
            }
        }
    }
    
    post {
        failure {
            slackSend color: 'danger', 
                      message: "Dashboard deployment failed: ${env.JOB_NAME} ${env.BUILD_NUMBER}"
        }
        success {
            slackSend color: 'good',
                      message: "Dashboards deployed successfully: ${env.JOB_NAME} ${env.BUILD_NUMBER}"
        }
    }
}
```

---

### Example 9: GitOps Dashboard Management

**Scenario:** Use ArgoCD/Flux for GitOps-style dashboard deployment.

**Repository Structure:**
```
kibana-gitops/
├── base/
│   ├── dashboards/
│   │   ├── manifest/
│   │   │   └── saved_objects.json
│   │   └── objects/
│   │       └── ...
│   └── kustomization.yaml
├── overlays/
│   ├── dev/
│   │   ├── .env
│   │   └── kustomization.yaml
│   ├── staging/
│   │   ├── .env
│   │   └── kustomization.yaml
│   └── prod/
│       ├── .env
│       └── kustomization.yaml
└── README.md
```

**Kubernetes CronJob (deploy-dashboards.yaml):**
```yaml
apiVersion: batch/v1
kind: CronJob
metadata:
  name: kibana-dashboard-sync
  namespace: observability
spec:
  schedule: "*/30 * * * *"  # Every 30 minutes
  jobTemplate:
    spec:
      template:
        spec:
          serviceAccountName: kibana-deployer
          containers:
          - name: kibob
            image: rust:1.75
            command:
            - /bin/bash
            - -c
            - |
              cargo install kibana-object-manager
              git clone https://github.com/yourorg/kibana-gitops.git
              cd kibana-gitops/base/dashboards
              kibob push . --managed true
            env:
            - name: KIBANA_URL
              valueFrom:
                secretKeyRef:
                  name: kibana-credentials
                  key: url
            - name: KIBANA_APIKEY
              valueFrom:
                secretKeyRef:
                  name: kibana-credentials
                  key: apikey
          restartPolicy: OnFailure
```

---

### Example 10: Terraform Integration

**Scenario:** Manage Kibana dashboards with Terraform.

**main.tf:**
```hcl
resource "null_resource" "kibana_dashboards" {
  triggers = {
    # Redeploy when manifest changes
    manifest_hash = filemd5("${path.module}/dashboards/manifest/saved_objects.json")
  }
  
  provisioner "local-exec" {
    command = <<-EOT
      export KIBANA_URL="${var.kibana_url}"
      export KIBANA_APIKEY="${var.kibana_apikey}"
      kibob auth
      kibob push ${path.module}/dashboards --managed true
    EOT
  }
}

variable "kibana_url" {
  description = "Kibana URL"
  type        = string
}

variable "kibana_apikey" {
  description = "Kibana API Key"
  type        = string
  sensitive   = true
}

output "dashboard_sync" {
  value = "Dashboards synchronized"
  depends_on = [null_resource.kibana_dashboards]
}
```

**Usage:**
```bash
terraform init
terraform plan -var="kibana_url=https://kibana.example.com" \
               -var="kibana_apikey=$KIBANA_APIKEY"
terraform apply
```

---

## Advanced Scenarios

### Example 11: Selective Object Management

**Scenario:** Manage only production dashboards, exclude development experiments.

```bash
# Start with all objects
kibob init export.ndjson ./dashboards

# Edit manifest to remove experimental dashboards
vim dashboards/manifest/saved_objects.json
# Remove objects with "type": "dashboard" and title containing "[DEV]"

# Alternative: Use jq to filter programmatically
jq '.objects |= map(select(
  .type != "dashboard" or 
  (.attributes.title | contains("[DEV]") | not)
))' dashboards/manifest/saved_objects.json > filtered.json
mv filtered.json dashboards/manifest/saved_objects.json

# Now only tracked objects are managed
kibob pull ./dashboards
kibob push ./dashboards --managed true
```

---

### Example 12: Dashboard Templating

**Scenario:** Create multiple similar dashboards from a template.

```bash
# 1. Create template dashboard
kibob pull ./template

# 2. Copy and modify for different teams
for team in alpha beta gamma; do
  mkdir -p ./dashboards-${team}
  cp -r template/manifest ./dashboards-${team}/
  cp -r template/objects ./dashboards-${team}/
  
  # Update dashboard titles
  find ./dashboards-${team}/objects -name "*.json" -type f -exec \
    sed -i '' "s/Team Template/Team ${team^}/g" {} \;
  
  # Generate new IDs
  for file in ./dashboards-${team}/objects/dashboard/*.json; do
    new_id="${team}-$(uuidgen | tr '[:upper:]' '[:lower:]')"
    old_id=$(basename "$file" .json)
    sed -i '' "s/$old_id/$new_id/g" "$file"
    mv "$file" "./dashboards-${team}/objects/dashboard/${new_id}.json"
  done
  
  # Deploy
  export KIBANA_SPACE="team-${team}"
  kibob push ./dashboards-${team} --managed false
done
```

---

### Example 13: Automated Dashboard Testing

**Scenario:** Validate dashboards before deployment.

**validate-dashboards.sh:**
```bash
#!/bin/bash
set -e

DASHBOARDS_DIR="./dashboards"

echo "Validating dashboard structure..."

# Check manifest exists
if [ ! -f "$DASHBOARDS_DIR/manifest/saved_objects.json" ]; then
  echo "Error: Manifest not found"
  exit 1
fi

# Validate manifest JSON
if ! jq empty "$DASHBOARDS_DIR/manifest/saved_objects.json"; then
  echo "Error: Invalid manifest JSON"
  exit 1
fi

# Check all referenced objects exist
missing_count=0
while IFS= read -r object; do
  type=$(echo "$object" | jq -r '.type')
  id=$(echo "$object" | jq -r '.id')
  file="$DASHBOARDS_DIR/objects/$type/$id.json"
  
  if [ ! -f "$file" ]; then
    echo "Error: Missing object file: $file"
    ((missing_count++))
  fi
done < <(jq -c '.objects[]' "$DASHBOARDS_DIR/manifest/saved_objects.json")

if [ $missing_count -gt 0 ]; then
  echo "Validation failed: $missing_count missing objects"
  exit 1
fi

echo "✓ Dashboard validation passed"

# Test deployment to dev environment
echo "Testing deployment to dev..."
export KIBANA_URL="${DEV_KIBANA_URL}"
export KIBANA_APIKEY="${DEV_KIBANA_APIKEY}"

kibob auth
kibob push "$DASHBOARDS_DIR" --managed false

echo "✓ Dev deployment successful"
```

**GitHub Actions workflow:**
```yaml
name: Validate Dashboards

on:
  pull_request:
    paths:
      - 'dashboards/**'

jobs:
  validate:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Install dependencies
        run: |
          cargo install kibana-object-manager
          sudo apt-get install -y jq
      
      - name: Validate dashboards
        env:
          DEV_KIBANA_URL: ${{ secrets.DEV_KIBANA_URL }}
          DEV_KIBANA_APIKEY: ${{ secrets.DEV_KIBANA_APIKEY }}
        run: ./validate-dashboards.sh
```

---

### Example 14: Dashboard as Code Review Process

**Scenario:** Require code review for dashboard changes.

**.github/CODEOWNERS:**
```
# Dashboard changes require review from platform team
/dashboards/ @yourorg/platform-team
```

**Review checklist template (.github/pull_request_template.md):**
```markdown
## Dashboard Changes Checklist

- [ ] All dashboard titles follow naming convention
- [ ] No sensitive data in filter defaults
- [ ] Time ranges are appropriate
- [ ] Visualizations have descriptions
- [ ] Index patterns are documented
- [ ] Changes tested in dev environment
- [ ] Screenshots attached showing changes

## Deployment Plan

- [ ] Deploy to dev
- [ ] Deploy to staging
- [ ] Deploy to production

## Rollback Plan

Git commit to rollback to: `________`
```

---

### Example 15: Disaster Recovery Drill

**Scenario:** Practice restoring from backup to validate recovery procedures.

```bash
#!/bin/bash
# disaster-recovery-drill.sh

set -e

echo "=== Kibana Dashboard Disaster Recovery Drill ==="

# 1. Document current state
export KIBANA_URL=https://prod-kibana.example.com
export KIBANA_APIKEY=prod_key

echo "Step 1: Backing up current state..."
mkdir -p ./dr-drill/before
kibob pull ./dr-drill/before
BEFORE_CHECKSUM=$(find ./dr-drill/before -type f -name "*.json" -exec md5sum {} \; | sort | md5sum)
echo "Before checksum: $BEFORE_CHECKSUM"

# 2. Simulate disaster (delete a dashboard)
echo "Step 2: Simulating disaster (deleting dashboard)..."
# (Manual step - delete a dashboard in Kibana UI)
read -p "Delete a dashboard in Kibana UI, then press Enter..."

# 3. Verify damage
echo "Step 3: Verifying damage..."
mkdir -p ./dr-drill/damaged
kibob pull ./dr-drill/damaged
DAMAGED_CHECKSUM=$(find ./dr-drill/damaged -type f -name "*.json" -exec md5sum {} \; | sort | md5sum)
echo "Damaged checksum: $DAMAGED_CHECKSUM"

if [ "$BEFORE_CHECKSUM" == "$DAMAGED_CHECKSUM" ]; then
  echo "Error: No damage detected. Delete a dashboard and try again."
  exit 1
fi

# 4. Restore from Git
echo "Step 4: Restoring from Git..."
git clone https://github.com/yourorg/kibana-dashboards.git ./dr-drill/restore
cd ./dr-drill/restore
kibob push . --managed true

# 5. Verify restoration
echo "Step 5: Verifying restoration..."
cd ../..
mkdir -p ./dr-drill/after
kibob pull ./dr-drill/after
AFTER_CHECKSUM=$(find ./dr-drill/after -type f -name "*.json" -exec md5sum {} \; | sort | md5sum)
echo "After checksum: $AFTER_CHECKSUM"

if [ "$BEFORE_CHECKSUM" == "$AFTER_CHECKSUM" ]; then
  echo "✓ Disaster recovery successful!"
  echo "Recovery time: $SECONDS seconds"
else
  echo "✗ Recovery verification failed"
  exit 1
fi

# 6. Cleanup
echo "Step 6: Cleaning up drill artifacts..."
rm -rf ./dr-drill

echo "=== Drill Complete ==="
```

---

## Tips and Tricks

### Tip 1: Use Shell Aliases for Common Operations

```bash
# Add to ~/.bashrc or ~/.zshrc
alias kdev='export KIBANA_URL=http://localhost:5601 KIBANA_USERNAME=elastic KIBANA_PASSWORD=changeme'
alias kprod='export KIBANA_URL=https://prod.example.com KIBANA_APIKEY=$(pass kibana/prod)'
alias kpull='kibob pull .'
alias kpush='kibob push . --managed true'
alias kdiff='kibob pull . && git diff objects/'
```

### Tip 2: Pre-commit Hook for Validation

```bash
# .git/hooks/pre-commit
#!/bin/bash
if git diff --cached --name-only | grep -q "^dashboards/"; then
  echo "Validating dashboard changes..."
  jq empty dashboards/manifest/saved_objects.json || exit 1
  echo "✓ Dashboard validation passed"
fi
```

### Tip 3: Monitor Dashboard Drift

```bash
#!/bin/bash
# check-dashboard-drift.sh
# Run daily to detect unauthorized changes

kibob pull ./current
if ! diff -r ./dashboards ./current > /dev/null; then
  echo "WARNING: Dashboard drift detected!"
  diff -r ./dashboards ./current | mail -s "Dashboard Drift Alert" ops@example.com
fi
```

---

Need more help? Check out:
- [User Guide](USER_GUIDE.md) - Complete command reference
- [Architecture](ARCHITECTURE.md) - How kibob works internally
- [Contributing](../CONTRIBUTING.md) - Help improve kibob
