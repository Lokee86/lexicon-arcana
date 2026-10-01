$ErrorActionPreference = 'Stop'

$repo = 'C:\!bin\workspace\lexicon-arcana-heap\arcana'
$exe = Join-Path $repo 'target\release\arcana.exe'
$lexicon = 'C:\!bin\workspace\hermes-agent\.lexicon'
$resultDir = Join-Path $repo 'evaluation\results\hermes-arcana-stage-c-gate-2026-09-28'
$state = Join-Path $env:TEMP 'arcana-stage-c-gate-state'
$stdout = Join-Path $resultDir 'sync.stdout.txt'
$stderr = Join-Path $resultDir 'sync.stderr.txt'

New-Item -ItemType Directory -Force -Path $resultDir | Out-Null
if (Test-Path $state) { Remove-Item -Recurse -Force $state }
Remove-Item -Force -ErrorAction SilentlyContinue $stdout, $stderr

$current = (Get-Content (Join-Path $lexicon 'CURRENT') -Raw).Trim()
$oracleSnapshot = 'sha256:ee46a8a475c443f908e0eff6db9efca62e3871fe1f541e62251ebd9f5ed2c621'
if ($current -ne $oracleSnapshot) { throw "Unexpected Hermes Lexicon snapshot: $current" }

$oldProfile = $env:ARCANA_SYNC_PROFILE
$env:ARCANA_SYNC_PROFILE = '1'
$stopwatch = [System.Diagnostics.Stopwatch]::StartNew()
$process = Start-Process -FilePath $exe -ArgumentList @('sync', '--lexicon', $lexicon, '--state', $state) -RedirectStandardOutput $stdout -RedirectStandardError $stderr -PassThru -NoNewWindow

$peak = [int64]0
$samples = 0
$childObserved = $false
$trace = [System.Collections.Generic.List[string]]::new()
$trace.Add('elapsed_ms,rss_bytes')
while (-not $process.HasExited) {
    try {
        $process.Refresh()
        $rss = [int64]$process.WorkingSet64
        $trace.Add(('{0:F3},{1}' -f $stopwatch.Elapsed.TotalMilliseconds, $rss))
        if ($rss -gt $peak) { $peak = $rss }
    } catch {}

    if (($samples % 250) -eq 0) {
        try {
            $children = @(Get-CimInstance Win32_Process -Filter "ParentProcessId = $($process.Id)" -ErrorAction Stop)
            if ($children.Count -gt 0) {
                $childObserved = $true
                $tree = [int64]$rss
                foreach ($child in $children) {
                    try { $tree += [int64](Get-Process -Id $child.ProcessId -ErrorAction Stop).WorkingSet64 } catch {}
                }
                if ($tree -gt $peak) { $peak = $tree }
            }
        } catch {}
    }

    $samples++
    Start-Sleep -Milliseconds 20
}

$process.WaitForExit()
$process.Refresh()
$stopwatch.Stop()
if ($null -eq $oldProfile) {
    Remove-Item Env:ARCANA_SYNC_PROFILE -ErrorAction SilentlyContinue
} else {
    $env:ARCANA_SYNC_PROFILE = $oldProfile
}

$exitCode = if ($null -eq $process.ExitCode) { 0 } else { [int]$process.ExitCode }
if ($exitCode -ne 0) {
    Get-Content $stderr
    throw "Arcana sync failed with exit code $exitCode"
}

$trace | Set-Content -Encoding UTF8 (Join-Path $resultDir 'rss-trace.csv')
$samplePath = Join-Path $resultDir 'rss-sample.json'
[ordered]@{
    peak_process_tree_rss_bytes = $peak
    peak_process_tree_rss_mib = [math]::Round($peak / 1MB, 2)
    rss_sampling_interval_ms = 20
    rss_samples = $samples
    child_process_observed = $childObserved
} | ConvertTo-Json | Set-Content -Encoding UTF8 $samplePath

$digest = $current.Substring(7)
$snapshotDir = Join-Path $state ('snapshots\' + $digest)
$repositoryFile = Join-Path $snapshotDir 'repository.arcana'
$graphFile = Join-Path $snapshotDir 'graph.arcana'
$manifest = @{}
Get-Content (Join-Path $snapshotDir 'repository.manifest') | ForEach-Object {
    if ($_ -match '=') {
        $parts = $_.Split('=', 2)
        $manifest[$parts[0]] = $parts[1]
    }
}

function Get-Sha256Hex([string]$path) {
    $sha = [System.Security.Cryptography.SHA256]::Create()
    $stream = [System.IO.File]::OpenRead($path)
    try {
        $digestBytes = $sha.ComputeHash($stream)
        return ([BitConverter]::ToString($digestBytes)).Replace('-', '').ToLowerInvariant()
    } finally {
        $stream.Dispose()
        $sha.Dispose()
    }
}

$repositoryHash = Get-Sha256Hex $repositoryFile
$graphHash = Get-Sha256Hex $graphFile
$repositorySize = (Get-Item $repositoryFile).Length
$graphSize = (Get-Item $graphFile).Length
$profile = (Get-Content $stderr -Raw).Trim()

$result = [ordered]@{
    gate = 'Stage C'
    branch = 'perf/arcana-heap-refactor'
    commit = (git -C 'C:\!bin\workspace\lexicon-arcana-heap' rev-parse HEAD).Trim()
    lexicon_snapshot = $current
    rescan_performed = $false
    release_binary = $true
    mode = 'rebuild'
    exit_code = $exitCode
    wall_seconds = [math]::Round($stopwatch.Elapsed.TotalSeconds, 6)
    peak_process_tree_rss_bytes = $peak
    peak_process_tree_rss_mib = [math]::Round($peak / 1MB, 2)
    rss_sampling_interval_ms = 20
    rss_samples = $samples
    child_process_observed = $childObserved
    counts = [ordered]@{
        nodes = [int64]$manifest['node_count']
        visible_edges = [int64]$manifest['edge_count']
        unresolved = [int64]$manifest['unresolved_count']
    }
    artifacts = [ordered]@{
        'repository.arcana' = [ordered]@{
            size_bytes = $repositorySize
            sha256 = $repositoryHash
            oracle_byte_identical = ($repositorySize -eq 489778032 -and $repositoryHash -eq '10cb311318a28703e3c9a510cae1b177e24f123c2d604ffafc194c3e55de0281')
        }
        'graph.arcana' = [ordered]@{
            size_bytes = $graphSize
            sha256 = $graphHash
            oracle_byte_identical = ($graphSize -eq 46411248 -and $graphHash -eq 'ee64b0367905d5e39c64d75c1269b429fc3791b5a6287cce0576e32f22e8fd5d')
        }
    }
    profile_stderr = $profile
}

$jsonPath = Join-Path $resultDir 'stage-c.json'
$result | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 $jsonPath
$result | ConvertTo-Json -Depth 8
Remove-Item -Recurse -Force $state