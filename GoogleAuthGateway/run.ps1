# Windows entry point. Node reads the private env file; PowerShell never evaluates its contents.
param([string]$Action = "start", [Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$ErrorActionPreference = "Stop"
$GatewayRoot = $PSScriptRoot
$ConfigRoot = if ($env:XDG_CONFIG_HOME) { $env:XDG_CONFIG_HOME } else { Join-Path $HOME ".config" }
$ConfigFile = Join-Path $ConfigRoot "aosp-wear-gateway/config.env"
if ($Action -in @("init","provision")) {
    $PythonCommand = Get-Command python3, python -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $PythonCommand) { throw "Install Python 3 first" }
    $Script = if ($Action -eq "init") { "configure.py" } else { "provision_watch.py" }
    & $PythonCommand.Source (Join-Path $GatewayRoot $Script) @Arguments
    exit $LASTEXITCODE
}
if ($Action -eq "test") {
    $TestFiles = @(Get-ChildItem (Join-Path $GatewayRoot "tests/*.test.mjs") | ForEach-Object { $_.FullName })
    & node --test @TestFiles
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
    $PythonCommand = Get-Command python3, python -ErrorAction SilentlyContinue | Select-Object -First 1
    if (-not $PythonCommand) { throw "Install Python 3 first" }
    & $PythonCommand.Source -m unittest discover -s (Join-Path $GatewayRoot "tests") -p "test_*.py"
    exit $LASTEXITCODE
}
if (-not (Test-Path $ConfigFile)) { throw "Run init OAUTH_JSON first; never paste secrets into source" }
$LocalCA = Join-Path (Split-Path $ConfigFile) "local-ca.pem"
if (Test-Path $LocalCA) { $env:NODE_EXTRA_CA_CERTS = $LocalCA }
$env:GATEWAY_CONFIG_FILE = $ConfigFile
$NodeCheck = "const [major,minor]=process.versions.node.split('.').map(Number);if(major<22||(major===22&&minor<13))throw Error('Node.js 22.13+ required');require('node:sqlite')"
& node -e $NodeCheck
if ($LASTEXITCODE -ne 0) { throw "Node.js 22.13+ with node:sqlite is required" }
if ($Action -eq "ca") {
    & node "--env-file=$ConfigFile" (Join-Path $GatewayRoot "src/export-ca.mjs")
} elseif ($Action -eq "providers") {
    & node (Join-Path $GatewayRoot "src/configure-providers.mjs") @Arguments
} elseif ($Action -eq "check") {
    & node "--env-file=$ConfigFile" (Join-Path $GatewayRoot "src/check.mjs")
} elseif ($Action -eq "start") {
    & node "--env-file=$ConfigFile" (Join-Path $GatewayRoot "src/server.mjs")
} elseif ($Action -in @("register", "pair", "poll", "confirm", "renew", "logout")) {
    & node "--env-file=$ConfigFile" (Join-Path $GatewayRoot "src/admin.mjs") $Action @Arguments
} else { throw "Commands: init, start, check, test, register, pair, poll, confirm, renew, logout" }
exit $LASTEXITCODE
