[CmdletBinding()]
param(
    [string]$OutDir = "core",
    [string]$Version = "v1.14.4",
    [string]$ExpectedZipSha256 = "99185d75d314a07dbba4a130c8ad986cbeb629a07927975636b341ca61f991df",
    [string]$ExpectedExeSha256 = "8f30682ee198855440d2a7fe070d5d3653793b2d01bb8c85eea88f93bbe7c8b9"
)

$ErrorActionPreference = "Stop"

$verNum = $Version.TrimStart("v")
$assetName = "sing-box-$verNum-windows-amd64.zip"
$officialUrl = "https://github.com/SagerNet/sing-box/releases/download/$Version/$assetName"
$candidateUrls = @(
    $officialUrl,
    "https://ghfast.top/$officialUrl",
    "https://gh-proxy.com/$officialUrl"
)

New-Item -ItemType Directory -Force -Path $OutDir | Out-Null
$destExe = Join-Path (Resolve-Path $OutDir).Path "sing-box.exe"

if (Test-Path $destExe) {
    $curHash = (Get-FileHash -Algorithm SHA256 $destExe).Hash.ToLowerInvariant()
    if ($curHash -eq $ExpectedExeSha256.ToLowerInvariant()) {
        Write-Host "OK: verified official sing-box $Version already present at $destExe"
        exit 0
    }
}

$tmpDir = Join-Path ([System.IO.Path]::GetTempPath()) ("portline-fetch-core-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Force -Path $tmpDir | Out-Null
$tmpZip = Join-Path $tmpDir $assetName

try {
    $downloaded = $false
    foreach ($url in $candidateUrls) {
        try {
            Write-Host "Fetching official sing-box $Version from $url ..."
            Invoke-WebRequest -Uri $url -OutFile $tmpZip -TimeoutSec 45 -UseBasicParsing
            $gotZipHash = (Get-FileHash -Algorithm SHA256 $tmpZip).Hash.ToLowerInvariant()
            if ($gotZipHash -ne $ExpectedZipSha256.ToLowerInvariant()) {
                Write-Warning "SHA-256 mismatch from $url (got $gotZipHash); rejecting and trying next source."
                Remove-Item -Force $tmpZip -ErrorAction SilentlyContinue
                continue
            }
            $downloaded = $true
            break
        } catch {
            Write-Warning "Download failed from $url : $($_.Exception.Message)"
        }
    }

    if (-not $downloaded) {
        throw "Failed to download and verify official sing-box $Version from all channels."
    }

    Expand-Archive -Path $tmpZip -DestinationPath $tmpDir -Force
    $extractedExe = Get-ChildItem -Path $tmpDir -Recurse -Filter "sing-box.exe" | Select-Object -First 1
    if (-not $extractedExe) {
        throw "sing-box.exe not found inside $assetName"
    }

    $gotExeHash = (Get-FileHash -Algorithm SHA256 $extractedExe.FullName).Hash.ToLowerInvariant()
    if ($gotExeHash -ne $ExpectedExeSha256.ToLowerInvariant()) {
        throw "Extracted sing-box.exe SHA-256 mismatch: expected $ExpectedExeSha256, got $gotExeHash"
    }

    Copy-Item -Path $extractedExe.FullName -Destination $destExe -Force
    Write-Host "OK: installed verified official sing-box $Version -> $destExe (sha256: $gotExeHash)"
} finally {
    Remove-Item -Recurse -Force $tmpDir -ErrorAction SilentlyContinue
}
