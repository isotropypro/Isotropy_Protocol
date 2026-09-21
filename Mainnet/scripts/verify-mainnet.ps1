param(
    [string]$Lcd = "https://terra-classic-lcd.publicnode.com",
    [string]$ArtifactsDir = ".\artifacts"
)

Set-StrictMode -Version Latest
$ErrorActionPreference = "Stop"
$Lcd = $Lcd.TrimEnd('/')
$repoRoot = Split-Path -Parent $PSScriptRoot
$manifest = Get-Content (Join-Path $repoRoot 'verification-manifest.json') -Raw | ConvertFrom-Json
$artifactRoot = Join-Path $repoRoot $ArtifactsDir

function Read-Lcd([string]$Path) {
    Invoke-RestMethod -Uri "$Lcd$Path" -TimeoutSec 30
}
function Get-BytesHash([byte[]]$Bytes) {
    $sha = [Security.Cryptography.SHA256]::Create()
    try { ([BitConverter]::ToString($sha.ComputeHash($Bytes))).Replace('-', '').ToLowerInvariant() }
    finally { $sha.Dispose() }
}
function Read-Smart([string]$Address, [string]$Query) {
    $json = '{"' + $Query + '":{}}'
    $encoded = [Uri]::EscapeDataString([Convert]::ToBase64String([Text.Encoding]::UTF8.GetBytes($json)))
    (Read-Lcd "/cosmwasm/wasm/v1/contract/$Address/smart/$encoded").data
}

$node = Read-Lcd '/cosmos/base/tendermint/v1beta1/node_info'
if ($node.default_node_info.network -ne $manifest.chain_id) { throw 'Unexpected chain ID' }
$before = (Read-Lcd '/cosmos/base/tendermint/v1beta1/blocks/latest').block.header
$checks = foreach ($contract in $manifest.contracts) {
    $info = (Read-Lcd "/cosmwasm/wasm/v1/contract/$($contract.address)").contract_info
    if ([string]$info.code_id -ne $contract.code_id) {
        throw "Active code ID changed for $($contract.name); review the migration before updating the manifest."
    }
    $code = Read-Lcd "/cosmwasm/wasm/v1/code/$($info.code_id)"
    $binaryHash = Get-BytesHash ([Convert]::FromBase64String($code.data))
    $artifactHash = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $artifactRoot $contract.artifact)).Hash.ToLowerInvariant()
    if ($binaryHash -ne $contract.sha256 -or $artifactHash -ne $contract.sha256 -or
        $code.code_info.data_hash.ToLowerInvariant() -ne $contract.sha256) {
        throw "WASM checksum mismatch for $($contract.name)"
    }
    [pscustomobject]@{ name=$contract.name; address=$contract.address; code_id=$info.code_id; sha256=$binaryHash; admin=$info.admin }
}
$controller = $manifest.contracts[0].address
$config = Read-Smart $controller 'config'
if ($config.token_address -ne $manifest.contracts[1].address) { throw 'Controller token address differs from manifest' }
$cycle = Read-Smart $controller 'current_cycle'
$state = Read-Smart $controller 'global_state'
$after = (Read-Lcd '/cosmos/base/tendermint/v1beta1/blocks/latest').block.header

# Live state is reported, not compared against a date that may legitimately change.
# These latest-state requests span a height range; they are not an atomic snapshot.
[pscustomobject]@{
    checked_at_utc=[DateTimeOffset]::UtcNow.ToString('o')
    lcd=$Lcd; chain_id=$manifest.chain_id
    observed_height_before=$before.height; observed_height_after=$after.height
    contracts=$checks; config=$config; current_cycle=$cycle; global_state=$state
    current_cycle_start_utc=[DateTimeOffset]::FromUnixTimeSeconds([long]$cycle.start_time).ToString('u')
} | ConvertTo-Json -Depth 12
