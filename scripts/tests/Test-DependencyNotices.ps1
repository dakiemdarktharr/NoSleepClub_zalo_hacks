#Requires -Version 7.0
$ErrorActionPreference='Stop'
$noraRepository=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '../..'))
$noraFixture=Join-Path $noraRepository ('target/notice-copy-tests/'+[guid]::NewGuid().ToString('N'))
$noraSource=Join-Path $noraFixture 'source'
[IO.Directory]::CreateDirectory((Join-Path $noraSource 'texts')) | Out-Null
$noraLock=Join-Path $noraFixture 'Cargo.lock'
[IO.File]::WriteAllText($noraLock,"version = 4`n")
$noraLockHash=(Get-FileHash -LiteralPath $noraLock).Hash.ToLowerInvariant()
$noraBytes=[Text.Encoding]::UTF8.GetBytes('Synthetic notice fixture')
$noraTextHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($noraBytes)).ToLowerInvariant()
$noraTextPath=Join-Path $noraSource ('texts/'+$noraTextHash+'.txt')
[IO.File]::WriteAllBytes($noraTextPath,$noraBytes)
$noraEntry=@{path=('texts/'+$noraTextHash+'.txt');bytes=$noraBytes.Length;sha256=$noraTextHash}
$noraInventory=@{schema_version=1;target='x86_64-pc-windows-msvc';lock_sha256=$noraLockHash;redistribution_cleared=$false;packages=@(@{texts=@($noraEntry)});gaps=@()}
function Save-NoraFixture {
 [IO.File]::WriteAllText((Join-Path $noraSource 'inventory.json'),($noraInventory | ConvertTo-Json -Depth 10))
}
Save-NoraFixture
@{bomFormat='CycloneDX';specVersion='1.6';metadata=@{properties=@(@{name='nora:cargo:lock-sha256';value=$noraLockHash})}} | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $noraSource 'bom.cdx.json')
$noraCopier=Join-Path $noraRepository 'scripts/Copy-DependencyNotices.ps1'
$noraDestination=Join-Path $noraFixture 'copied'
& $noraCopier -Source $noraSource -Destination $noraDestination -Lockfile $noraLock | Out-Null
if((Get-FileHash -LiteralPath (Join-Path $noraDestination $noraEntry.path)).Hash -ne $noraTextHash){throw 'Valid copy did not preserve bytes'}
function Assert-NoraRejected([string]$Case) {
 $noraRejected=$false
 try {& $noraCopier -Source $noraSource -Destination (Join-Path $noraFixture $Case) -Lockfile $noraLock | Out-Null}catch{$noraRejected=$true}
 if(-not $noraRejected -or (Test-Path -LiteralPath (Join-Path $noraFixture $Case))){throw "Unsafe case was not rejected before writes: $Case"}
}
[IO.File]::WriteAllText($noraTextPath,'tampered')
Assert-NoraRejected 'tampered'
[IO.File]::WriteAllBytes($noraTextPath,$noraBytes)
$noraEntry.path='../.env.local'
[IO.File]::WriteAllText((Join-Path $noraFixture '.env.local'),'synthetic private content')
Save-NoraFixture
Assert-NoraRejected 'escaped'
$noraEntry.path='texts/'+$noraTextHash+'.txt'
$noraInventory.lock_sha256='0'*64
Save-NoraFixture
Assert-NoraRejected 'stale-lock'
Write-Output 'Notice copy checks passed: byte-preserving copy, tamper rejection, path escape rejection, stale-lock rejection.'
