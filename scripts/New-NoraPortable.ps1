#Requires -Version 7.0
[CmdletBinding()]
param(
 [Parameter(Mandatory)][string]$Executable,
 [Parameter(Mandatory)][string]$AssetSource,
 [Parameter(Mandatory)][string]$Destination,
 [string]$LlmConfig,
 [switch]$HardlinkAssets
)
$ErrorActionPreference='Stop'
$noraRepository=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$noraDestination=[IO.Path]::GetFullPath($Destination)
$noraSource=[IO.Path]::GetFullPath($AssetSource)
if(Test-Path -LiteralPath $noraDestination){throw 'Portable destination must be new'}
if($noraDestination.StartsWith($noraSource+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Destination cannot be nested in source assets'}
& (Join-Path $PSScriptRoot 'Prepare-VoiceAssets.ps1') -Destination $noraSource -VerifyOnly | Out-Null
foreach($noraDir in @('bin','assets','config','licenses')){[IO.Directory]::CreateDirectory((Join-Path $noraDestination $noraDir)) | Out-Null}
[IO.File]::Copy([IO.Path]::GetFullPath($Executable),(Join-Path $noraDestination 'bin/nora-app.exe'),$false)
$noraPacks=@(@('vieneu-v3-turbo-fp32.json','vieneu'),@('silero-vad.json','silero'),@('sea-g2p.json','g2p'),@('whisper-large-v3-turbo-q5_0.json','whisper'),@('onnxruntime-win-x64-cpu.json','ort'))
foreach($noraPack in $noraPacks){
 $noraManifest=Get-Content -LiteralPath (Join-Path $noraRepository ('assets/manifests/'+$noraPack[0])) -Raw | ConvertFrom-Json
 foreach($noraEntry in $noraManifest.files){
    $noraRelative=Join-Path $noraPack[1] $noraEntry.path
    $noraFrom=Join-Path $noraSource $noraRelative
    $noraTo=Join-Path $noraDestination ('assets/'+$noraRelative)
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($noraTo)) | Out-Null
    if($HardlinkAssets){New-Item -ItemType HardLink -Path $noraTo -Target $noraFrom | Out-Null}else{[IO.File]::Copy($noraFrom,$noraTo,$false)}
 }
}
[IO.File]::Copy((Join-Path $noraRepository '.env.example'),(Join-Path $noraDestination 'config/.env.local'),$false)
foreach($noraNotice in @('LICENSE','THIRD_PARTY_NOTICES.md')){[IO.File]::Copy((Join-Path $noraRepository $noraNotice),(Join-Path $noraDestination ('licenses/'+$noraNotice)),$false)}
foreach($noraNotice in @('NOTICE','LICENSE-APACHE')){[IO.File]::Copy((Join-Path $noraRepository ('crates/nora-vieneu/'+$noraNotice)),(Join-Path $noraDestination ('licenses/vieneu-'+$noraNotice)),$false)}
$noraKeyReference=if($LlmConfig){[IO.Path]::GetFullPath($LlmConfig)}else{'config/.env.local'}
$noraConfig=@{state_dir='state';asset_root='assets';llm_config=$noraKeyReference;quiet=$false;public_research=$false;max_recording_seconds=30}
[IO.File]::WriteAllText((Join-Path $noraDestination 'app.json'),($noraConfig | ConvertTo-Json))
[IO.File]::WriteAllText((Join-Path $noraDestination 'Launch-Nora.cmd'),"@echo off`r`nstart `"`" `"%~dp0bin\nora-app.exe`" --config `"%~dp0app.json`"`r`n")
[IO.File]::WriteAllText((Join-Path $noraDestination 'README.txt'),"NORA Windows x64 validation candidate.`r`nRun Launch-Nora.cmd. Hold Ctrl+Shift+Space to speak.`r`nLLM key/model: config/.env.local, unless app.json references an existing external private file.`r`nNo key contents were copied. Keep config and state private after use.`r`nRequires AVX2/F16C/FMA, OpenGL3.3-compatible drivers and Microsoft Visual C++ x64 runtime.`r`nUnsigned local package; clean-machine/signing/voice-corpus gates are documented in the source repository.`r`nModel/dictionary/voice and dependency rights remain separate from NORA's MIT license.`r`n")
$noraTotal=(Get-ChildItem -LiteralPath $noraDestination -File -Recurse | Measure-Object Length -Sum).Sum
if($noraTotal -gt 15000000000){throw 'Portable package exceeds disk target'}
[pscustomobject]@{install_directory=$noraDestination;logical_bytes=$noraTotal;within_15gb=($noraTotal -le 15000000000);api_key_contents_copied=$false;model_assets_verified=$true;binary_sha256=(Get-FileHash -LiteralPath (Join-Path $noraDestination 'bin/nora-app.exe')).Hash;distribution_status='unsigned_validation_candidate'} | ConvertTo-Json -Compress
