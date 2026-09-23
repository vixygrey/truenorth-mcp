# generate-allure-report — Reference

## Data Sources

The script reads five YAML sources from the project:

| Source           | Path                                               | Fields Used                                         |
| ---------------- | -------------------------------------------------- | --------------------------------------------------- |
| Execution status | `.agent/tasks/execution-status.yml`                | `groups`, `stories`, `development_status`           |
| Release plan     | `.agent/tasks/release-plan.yml`                    | `release.version`, `release.status`, `bugs` summary |
| Task groups      | `.agent/tasks/<capsule>/group.yml` + `-tasks.yaml` | Group metadata, task pass/fail counts               |
| Bug references   | `.agent/tasks/bugs.yml` (optional)                 | Bug counts by status and severity                   |

The external tracker owns bug detail. Cycle-time metrics are out of scope, so the
report reads no metrics file.

## Script Body

The report generator reads the `.agent/` cockpit and writes the Allure result files:

```bash
#!/usr/bin/env bash
set -euo pipefail
source "$(dirname "${BASH_SOURCE[0]}")/lib/python-env.sh"
ROOT="$(git rev-parse --show-toplevel 2>/dev/null)" || ROOT="$(dirname "${BASH_SOURCE[0]}")/.."

mkdir -p "$ROOT/allure-results"

$PYTHON - "$ROOT" <<'PY'
import json
import sys
import xml.etree.ElementTree as ET
from pathlib import Path

root = Path(sys.argv[1])
out = root / "allure-results"

# 1. Read the .agent/ cockpit files
exec_status_file = root / ".agent" / "tasks" / "execution-status.yml"
release_plan_file = root / ".agent" / "tasks" / "release-plan.yml"
bugs_file = root / ".agent" / "tasks" / "bugs.yml"

sys.path.insert(0, str(root / "scripts" / "lib"))
from simple_yaml import parse_simple_yaml

exec_status = parse_simple_yaml(exec_status_file.read_text()) if exec_status_file.exists() else {}
release_plan = parse_simple_yaml(release_plan_file.read_text()) if release_plan_file.exists() else {}
bugs_registry = parse_simple_yaml(bugs_file.read_text()) if bugs_file.exists() else {"bugs": []}

# Cycle-time metrics are out of scope, so there is no per-story cycle-time lookup.
ct_lookup = {}

# 2. Build JUnit XML
stories = exec_status.get("stories", {})

# Counts for testsuite attributes
total_stories = len(stories)
incomplete = sum(1 for s in stories.values() if isinstance(s, dict) and s.get("status") != "done")

testsuite = ET.Element("testsuite", {
    "name": "group-progress",
    "tests": str(total_stories),
    "failures": str(incomplete),
    "errors": "0",
    "skipped": "0",
})

for story_id in sorted(stories.keys()):
    story = stories[story_id]
    if not isinstance(story, dict):
        continue

    group_id = story.get("group", "unknown")
    title = story.get("title", story_id)
    bcps = story.get("bcps", 0)
    status = story.get("status", "backlog")
    risk_max = story.get("risk_max", "none")
    security_max = story.get("security_max", "none")

    # Cycle-time metrics are out of scope, so the Allure time is left at zero.
    time_seconds = 0.0

    testcase = ET.SubElement(testsuite, "testcase", {
        "classname": group_id,
        "name": f"{story_id}: {title}",
        "time": str(round(time_seconds, 3)),
    })

    props = ET.SubElement(testcase, "properties")
    ET.SubElement(props, "property", {"name": "risk", "value": risk_max})
    ET.SubElement(props, "property", {"name": "security", "value": security_max})
    ET.SubElement(props, "property", {"name": "bcps", "value": str(bcps)})
    ET.SubElement(props, "property", {"name": "status", "value": status})

    if status != "done":
        ET.SubElement(testcase, "failure", {
            "message": f"Story {story_id} is {status} [risk={risk_max}, security={security_max}]",
            "type": "StoryIncomplete"
        })

tree = ET.ElementTree(testsuite)
ET.indent(tree, space="  ")
tree.write(str(out / "junit-results.xml"), encoding="utf-8", xml_declaration=True)

# 3. Build categories.json
groups = exec_status.get("groups", {})
categories = []

for group_id in sorted(groups.keys()):
    group = groups[group_id]
    if isinstance(group, dict) and group.get("status") != "done":
        categories.append({
            "name": f"Group: {group.get('title', group_id)}",
            "matchedStatuses": ["failed"],
            "messageRegex": f".*{group_id}:.*"
        })

categories.append({
    "name": "P0 Risk",
    "matchedStatuses": ["failed"],
    "messageRegex": ".*risk.*P0.*"
})
categories.append({
    "name": "Security Review",
    "matchedStatuses": ["failed"],
    "messageRegex": ".*security.*(?:medium|high).*"
})

# Add bug-based categories
bug_list = bugs_registry.get("bugs", [])
bug_count = len(bug_list) if isinstance(bug_list, list) else 0
open_bugs = sum(1 for b in bug_list if isinstance(b, dict) and b.get("status") not in ("fixed", "closed", None))
if open_bugs > 0:
    categories.append({
        "name": "Open Bugs",
        "matchedStatuses": ["failed"],
        "messageRegex": ".*Bug.*"
    })

(out / "categories.json").write_text(json.dumps(categories, indent=2))

# 4. Build executor.json
rl = release_plan.get("release", {}) if isinstance(release_plan.get("release"), dict) else {}
executor = {
    "name": "truenorth",
    "type": "truenorth",
    "buildName": rl.get("version", "unknown") if isinstance(rl, dict) else "unknown",
    "buildOrder": len(exec_status.get("development_status", {})),
}
(out / "executor.json").write_text(json.dumps(executor, indent=2))

# Summary
group_count = len([g for g in groups.values() if isinstance(g, dict) and g.get("status") == "done"])
total_groups = len(groups)
print(f"generate-allure-report: {total_stories} stories, {group_count}/{total_groups} groups done, {bug_count} bugs")
print(f"  -> {out}/junit-results.xml")
print(f"  -> {out}/categories.json")
print(f"  -> {out}/executor.json")
PY
```

## JUnit XML Schema

```xml
<?xml version="1.0" encoding="utf-8"?>
<testsuite name="group-progress" tests="N" failures="F" errors="0" skipped="0">
  <testcase classname="e01" name="e01s01: Security slopcheck tags" time="0.0">
    <properties>
      <property name="risk" value="none"/>
      <property name="security" value="none"/>
      <property name="bcps" value="1"/>
      <property name="status" value="done"/>
    </properties>
  </testcase>
  <testcase classname="e01" name="e01s99: Some incomplete story" time="0.0">
    <properties>...</properties>
    <failure message="Story e01s99 is backlog [risk=P0, security=high]" type="StoryIncomplete"/>
  </testcase>
</testsuite>
```

## Categories JSON Schema

```json
[
  {
    "name": "Group: Quality Core - Skill Hardening",
    "matchedStatuses": ["failed"],
    "messageRegex": ".*e45:.*"
  },
  {
    "name": "P0 Risk",
    "matchedStatuses": ["failed"],
    "messageRegex": ".*risk.*P0.*"
  },
  {
    "name": "Security Review",
    "matchedStatuses": ["failed"],
    "messageRegex": ".*security.*(?:medium|high).*"
  }
]
```

## Executor JSON Schema

```json
{
  "name": "truenorth",
  "type": "truenorth",
  "buildName": "2.76.2",
  "buildOrder": 400
}
```

## Example Usage

```bash
# Generate the Allure report from the .agent/ cockpit, then verify output
test -f allure-results/junit-results.xml && echo "JUnit OK"
test -f allure-results/categories.json && echo "Categories OK"
test -f allure-results/executor.json && echo "Executor OK"

# Serve with Allure
allure serve allure-results/

# Or open the Allure TestOps UI
allure open allure-results/
```
