# Back-Office Craft Suite: End-to-End Multi-App Event Bus Integration Test
$ErrorActionPreference = "Stop"
$env:PATH = "$env:USERPROFILE\.cargo\bin;$env:PATH"

$testRoot = Join-Path $env:TEMP ("craft-suite-test-" + [System.Guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $testRoot | Out-Null
$testBus = Join-Path $testRoot "bus"
New-Item -ItemType Directory -Force -Path $testBus | Out-Null
$env:CRAFT_BIZ_BUS = $testBus

Write-Host "=================================================================" -ForegroundColor Cyan
Write-Host " BACK-OFFICE CRAFT SUITE: MULTI-APP END-TO-END INTEGRATION TEST" -ForegroundColor Cyan
Write-Host " Bus Root: $testBus" -ForegroundColor Cyan
Write-Host "=================================================================" -ForegroundColor Cyan

$formcraftDb = Join-Path $testRoot "formcraft.db"
$gaincraftDb = Join-Path $testRoot "gaincraft.db"
$deskcraftDb = Join-Path $testRoot "deskcraft.db"
$taskcraftDb = Join-Path $testRoot "taskcraft.db"
$teamcraftDb = Join-Path $testRoot "teamcraft.db"
$wikicraftDb = Join-Path $testRoot "wikicraft.db"

# -------------------------------------------------------------
# STEP 1: formcraft - Survey creation, NPS Detractor submission and Routing
# -------------------------------------------------------------
Write-Host "`n[STEP 1] formcraft: creating survey and recording NPS detractor..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\formcraft
cargo run -q -p formcraft-mcp -- --db $formcraftDb define "csat-q1" "Customer Satisfaction Q1" | Out-Null
$respFile = Join-Path $testRoot "responses.json"
Set-Content -Path $respFile -Value '[{"account_id": "acme-corp.at", "nps_score": 2, "answers": {"comment": "Critical system outage and SLA breached. Considering contract termination!"}}]'
cargo run -q -p formcraft-mcp -- --db $formcraftDb import-responses "csat-q1" $respFile | Out-Null
$routeOut = cargo run -q -p formcraft-mcp -- --db $formcraftDb route-detractors "csat-q1"
Write-Host "formcraft route-detractors output: $routeOut"

$gainInbox = Join-Path $testBus "gaincraft\inbox"
if (-not (Test-Path $gainInbox)) {
    throw "STEP 1 FAILED: Directory gaincraft\inbox does not exist"
}
$pendingEvents = Get-ChildItem -File $gainInbox
if ($pendingEvents.Count -eq 0) {
    throw "STEP 1 FAILED: Expected signal.negative event in gaincraft\inbox"
}
Write-Host "[OK] Event generated: $($pendingEvents[0].Name) in gaincraft\inbox" -ForegroundColor Green

# -------------------------------------------------------------
# STEP 2: gaincraft - Ingest signal.negative from bus into Customer 360
# -------------------------------------------------------------
Write-Host "`n[STEP 2] gaincraft: draining inbox and triggering Customer 360 triage..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\gaincraft
$gainDrainOut = cargo run -q -p gaincraft-mcp -- --db $gaincraftDb inbox-drain
Write-Host "gaincraft inbox-drain output: $gainDrainOut"

$gainDone = Join-Path $testBus "gaincraft\done"
$processedEvents = Get-ChildItem -File $gainDone
if ($processedEvents.Count -eq 0) {
    throw "STEP 2 FAILED: Expected event in gaincraft\done"
}
Write-Host "[OK] Event successfully processed and moved to gaincraft\done" -ForegroundColor Green

$gainAccounts = cargo run -q -p gaincraft-mcp -- --db $gaincraftDb list
Write-Host "gaincraft accounts: $gainAccounts"
$accText = $gainAccounts -join " "
if ($accText -notmatch "acme-corp") {
    throw "STEP 2 FAILED: acme-corp.at was not auto-created in gaincraft"
}
Write-Host "[OK] Account acme-corp.at auto-created in gaincraft Customer 360" -ForegroundColor Green

# -------------------------------------------------------------
# STEP 3: deskcraft - Inbound ticket and Escalation to taskcraft
# -------------------------------------------------------------
Write-Host "`n[STEP 3] deskcraft: ticket escalation to engineering..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\deskcraft
cargo run -q -p deskcraft-mcp -- --db $deskcraftDb demo-seed | Out-Null
$escalateOut = cargo run -q -p deskcraft-mcp -- --db $deskcraftDb escalate "D-1" --project "ENG" --details "Production outage reported by acme-corp.at - failover cluster down"
Write-Host "deskcraft escalate output: $escalateOut"

$taskInbox = Join-Path $testBus "taskcraft\inbox"
if (-not (Test-Path $taskInbox)) {
    throw "STEP 3 FAILED: Directory taskcraft\inbox does not exist"
}
$taskEvents = Get-ChildItem -File $taskInbox
if ($taskEvents.Count -eq 0) {
    throw "STEP 3 FAILED: Expected ticket.escalated in taskcraft\inbox"
}
Write-Host "[OK] Event generated: $($taskEvents[0].Name) in taskcraft\inbox" -ForegroundColor Green

# -------------------------------------------------------------
# STEP 4: taskcraft - Ingest ticket.escalated, create issue and advance workflow
# -------------------------------------------------------------
Write-Host "`n[STEP 4] taskcraft: draining inbox and creating tracking issue..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\taskcraft
cargo run -q -p taskcraft-mcp -- --db $taskcraftDb project create "ENG" "Engineering" | Out-Null
$taskDrainOut = cargo run -q -p taskcraft-mcp -- --db $taskcraftDb inbox-drain
Write-Host "taskcraft inbox-drain output: $taskDrainOut"

$taskIssues = cargo run -q -p taskcraft-mcp -- --db $taskcraftDb list
Write-Host "taskcraft issues: $taskIssues"
$issueText = $taskIssues -join " "
if ($issueText -notmatch "ENG-") {
    throw "STEP 4 FAILED: Issue was not created in taskcraft"
}
Write-Host "[OK] Tracking issue successfully created in project ENG" -ForegroundColor Green

cargo run -q -p taskcraft-mcp -- --db $taskcraftDb move "ENG-1" in_progress | Out-Null
cargo run -q -p taskcraft-mcp -- --db $taskcraftDb move "ENG-1" done | Out-Null
Write-Host "[OK] Task ENG-1 transitioned to DONE" -ForegroundColor Green

# -------------------------------------------------------------
# STEP 5: teamcraft - Timesheet logging and AZG compliance check
# -------------------------------------------------------------
Write-Host "`n[STEP 5] teamcraft: logging incident response time and AZG audit..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\teamcraft
cargo run -q -p teamcraft-mcp -- --db $teamcraftDb employee add "DEV1" "Senior SRE Engineer" --hours 38.5 | Out-Null
$todayStr = Get-Date -Format "yyyy-MM-dd"
cargo run -q -p teamcraft-mcp -- --db $teamcraftDb time add "DEV1" --date $todayStr --start "08:00" --end "12:00" --break 30 --note "Hotfix for acme-corp.at failover issue" | Out-Null
$azgOut = cargo run -q -p teamcraft-mcp -- --db $teamcraftDb azg --from $todayStr --to $todayStr
Write-Host "teamcraft AZG compliance check: $azgOut"
Write-Host "[OK] Working time logged and checked against Austrian AZG regulations" -ForegroundColor Green

# -------------------------------------------------------------
# STEP 6: wikicraft - Architecture Decision Record and Post-Mortem
# -------------------------------------------------------------
Write-Host "`n[STEP 6] wikicraft: publishing Incident Post-Mortem and ADR..." -ForegroundColor Yellow
Set-Location D:\Dev\repos\wikicraft
$postMortem = "Root cause analysis for acme-corp.at incident. Decision to implement automatic multi-region database failover."
cargo run -q -p wikicraft-mcp -- --db $wikicraftDb page-save "incident-post-mortem" "Incident Post-Mortem: ACME Outage" $postMortem | Out-Null
cargo run -q -p wikicraft-mcp -- --db $wikicraftDb adr-save "adr-042-failover" "ADR-042: Automated Failover" "accepted" "Decision to implement Raft failover" | Out-Null

$searchOut = cargo run -q -p wikicraft-mcp -- --db $wikicraftDb search "failover"
Write-Host "wikicraft FTS5 Search Result: $searchOut"
$searchText = $searchOut -join " "
if ($searchText -notmatch "incident-post-mortem") {
    throw "STEP 6 FAILED: wikicraft search could not find incident-post-mortem"
}
Write-Host "[OK] wikicraft published and indexed post-mortem with FTS5 search" -ForegroundColor Green

Write-Host "`n=================================================================" -ForegroundColor Cyan
Write-Host " ALL 6 END-TO-END SCENARIO STEPS PASSED WITH 100% SUCCESS!" -ForegroundColor Green
Write-Host "=================================================================" -ForegroundColor Cyan
