#Requires -Version 7.0
[CmdletBinding()]
param(
 [Parameter(Mandatory)][string]$Source,
 [string]$Destination,
 [string]$Lockfile=(Join-Path $PSScriptRoot '../Cargo.lock'),
 [switch]$ValidateOnly
)
$ErrorActionPreference='Stop'
$noraNoticeRoot=[IO.Path]::GetFullPath($Source)
if(-not (Test-Path -LiteralPath $noraNoticeRoot -PathType Container) -or
   ((Get-Item -LiteralPath $noraNoticeRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Ordinary notice source directory required'}
function Read-NoraNoticeJson([string]$Name){
 $noraPath=Join-Path $noraNoticeRoot $Name
 $noraInfo=Get-Item -LiteralPath $noraPath
 if($noraInfo.Length -gt 2000000 -or ($noraInfo.Attributes -band [IO.FileAttributes]::ReparsePoint)){throw 'Invalid notice manifest'}
 return Get-Content -LiteralPath $noraPath -Raw | ConvertFrom-Json -Depth 100
}
$noraInventory=Read-NoraNoticeJson 'inventory.json'
$noraBom=Read-NoraNoticeJson 'bom.cdx.json'
$noraLockText=[IO.File]::ReadAllText([IO.Path]::GetFullPath($Lockfile)).Replace("`r`n","`n")
$noraLockHash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes($noraLockText)))
$noraBomLock=@($noraBom.metadata.properties | Where-Object {$_.name -eq 'nora:cargo:lock-sha256'})
if($noraInventory.schema_version -ne 1 -or $noraInventory.target -ne 'x86_64-pc-windows-msvc' -or
   $noraInventory.lock_sha256 -ne $noraLockHash -or $noraInventory.packages.Count -lt 1 -or $noraInventory.packages.Count -gt 1024 -or
   $noraInventory.redistribution_cleared -ne $false -or $noraBomLock.Count -ne 1 -or
   $noraBomLock[0].value -ne $noraLockHash -or $noraBom.bomFormat -ne 'CycloneDX' -or $noraBom.specVersion -ne '1.6'){throw 'Notice inventory does not match the locked candidate build'}
$noraFiles=@{};$noraTotal=0L
foreach($noraPackage in $noraInventory.packages){
 if($noraPackage.texts.Count -gt 256){throw 'Too many package notices'}
 foreach($noraText in $noraPackage.texts){
  if($noraText.path -cnotmatch '^texts/([0-9a-f]{64})\.txt$' -or $noraText.sha256 -cne $Matches[1] -or
     $noraText.bytes -le 0 -or $noraText.bytes -gt 1000000){throw 'Invalid notice file binding'}
  if($noraFiles.ContainsKey($noraText.path)){
   if($noraFiles[$noraText.path].bytes -ne $noraText.bytes){throw 'Conflicting notice binding'}
   continue
  }
  $noraFile=Join-Path $noraNoticeRoot $noraText.path
  foreach($noraCheck in @((Join-Path $noraNoticeRoot 'texts'),$noraFile)){
   if((Get-Item -LiteralPath $noraCheck).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Refusing linked notice files'}
  }
  $noraInfo=Get-Item -LiteralPath $noraFile
  if($noraInfo.Length -ne $noraText.bytes -or (Get-FileHash -LiteralPath $noraFile).Hash -ne $noraText.sha256){throw 'Notice integrity check failed'}
  $noraTotal+=[long]$noraText.bytes
  if($noraTotal -gt 20000000){throw 'Notice pack byte limit'}
  $noraFiles[$noraText.path]=$noraText
 }
}
if(-not $ValidateOnly){
 if(-not $Destination){throw 'New destination required'}
 $noraDestination=[IO.Path]::GetFullPath($Destination)
 if(Test-Path -LiteralPath $noraDestination){throw 'Notice destination must be new'}
 [IO.Directory]::CreateDirectory((Join-Path $noraDestination 'texts')) | Out-Null
 foreach($noraName in @('inventory.json','bom.cdx.json') + @($noraFiles.Keys | Sort-Object)){
  $noraTarget=Join-Path $noraDestination $noraName
  [IO.File]::Copy((Join-Path $noraNoticeRoot $noraName),$noraTarget,$false)
  if($noraFiles.ContainsKey($noraName) -and (Get-FileHash -LiteralPath $noraTarget).Hash -ne $noraFiles[$noraName].sha256){throw 'Notice changed during copy'}
 }
}
[pscustomobject]@{notice_texts=$noraFiles.Count;notice_text_bytes=$noraTotal;metadata_gaps=$noraInventory.gaps.Count;redistribution_cleared=$false;validated_only=[bool]$ValidateOnly} | ConvertTo-Json -Compress
