#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [Parameter(Mandatory)][string]$Config,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$InputWav,
    [ValidateRange(5,600)][int]$Seconds=120
)
$ErrorActionPreference='Stop'
$noraOutput=[IO.Path]::GetFullPath($OutputDirectory)
if(Test-Path -LiteralPath $noraOutput){throw 'Choose a new measurement directory'}
[IO.Directory]::CreateDirectory($noraOutput) | Out-Null
$noraMemory=Join-Path $noraOutput 'memory.json'
$noraInfo=[Diagnostics.ProcessStartInfo]::new()
$noraInfo.FileName=[IO.Path]::GetFullPath($Executable)
$noraInfo.UseShellExecute=$false
$noraInfo.RedirectStandardOutput=$true
$noraInfo.RedirectStandardError=$true
foreach($noraArg in @('--config',[IO.Path]::GetFullPath($Config),'--metrics',$noraMemory,'--exit-after',"$Seconds",'--ui-snapshot',(Join-Path $noraOutput 'nora.png'))){$noraInfo.ArgumentList.Add($noraArg)}
if($InputWav){$noraInfo.ArgumentList.Add('--import-audio');$noraInfo.ArgumentList.Add([IO.Path]::GetFullPath($InputWav))}
$noraCounters=@('\GPU Process Memory(*)\Dedicated Usage','\GPU Process Memory(*)\Shared Usage')
$noraGpuAvailable=$true
try{Get-Counter $noraCounters -ErrorAction Stop | Out-Null}catch{$noraGpuAvailable=$false}
$noraProcess=[Diagnostics.Process]::Start($noraInfo)
$noraStartedUtc=$noraProcess.StartTime.ToUniversalTime()
$noraStdout=$noraProcess.StandardOutput.ReadToEndAsync()
$noraStderr=$noraProcess.StandardError.ReadToEndAsync()
$noraPids=[Collections.Generic.HashSet[int]]::new();$noraPids.Add($noraProcess.Id) | Out-Null
$noraGpuSamples=0;$noraGpuErrors=0;$noraDedicatedPeak=0L;$noraSharedPeak=0L;$noraForced=$false
$noraClock=[Diagnostics.Stopwatch]::StartNew()
try{
 while(-not $noraProcess.HasExited){
    if(Test-Path -LiteralPath $noraMemory){try{$noraSnapshot=Get-Content -LiteralPath $noraMemory -Raw | ConvertFrom-Json;foreach($noraId in $noraSnapshot.observed_process_ids){$noraPids.Add([int]$noraId) | Out-Null}}catch{}}
    if($noraGpuAvailable){
        try{
            $noraGpu=Get-Counter $noraCounters -ErrorAction Stop
            $noraDedicated=0L;$noraShared=0L
            foreach($noraCounter in $noraGpu.CounterSamples){
                if($noraCounter.InstanceName -match '^pid_(\d+)_' -and $noraPids.Contains([int]$Matches[1])){
                    if($noraCounter.Status -ne 0){throw 'GPU counter status invalid'}
                    if($noraCounter.Path -like '*\dedicated usage'){$noraDedicated+=[long]$noraCounter.CookedValue}else{$noraShared+=[long]$noraCounter.CookedValue}
                }
            }
            $noraDedicatedPeak=[Math]::Max($noraDedicatedPeak,$noraDedicated);$noraSharedPeak=[Math]::Max($noraSharedPeak,$noraShared);$noraGpuSamples++
        }catch{if(-not $noraProcess.HasExited){$noraGpuErrors++}}
    }
    if($noraClock.Elapsed.TotalSeconds -gt $Seconds+30){$noraForced=$true;$noraProcess.Kill($true);break}
    Start-Sleep -Milliseconds 100
 }
 $noraProcess.WaitForExit()
 [IO.File]::WriteAllText((Join-Path $noraOutput 'stdout.txt'),$noraStdout.GetAwaiter().GetResult())
 [IO.File]::WriteAllText((Join-Path $noraOutput 'stderr.txt'),$noraStderr.GetAwaiter().GetResult())
 $noraReport=[ordered]@{
    schema_version=1
    started_utc=$noraStartedUtc.ToString('o')
    scope='Whole GUI and NORA Job process set; synthetic WAV if selected; external approved user applications and OS excluded. Inspect durable goal/playback results separately.'
    memory_report='memory.json'
    gpu_requested_sleep_ms=100
    gpu_average_sample_interval_ms=if($noraGpuSamples -gt 0){[math]::Round($noraClock.ElapsedMilliseconds/$noraGpuSamples,2)}else{$null}
    gpu_counter_available=$noraGpuAvailable
    gpu_samples=$noraGpuSamples
    gpu_errors=$noraGpuErrors
    gpu_peak_dedicated_bytes=$noraDedicatedPeak
    gpu_peak_shared_bytes=$noraSharedPeak
    gpu_observed_within_4gb=($noraGpuAvailable -and $noraGpuSamples -gt 0 -and $noraGpuErrors -eq 0 -and $noraDedicatedPeak -le 4000000000)
    gpu_note='All adapters summed. Lifetime observed Job PIDs are used; PID reuse could overcount. Sampling is not a physical4GB-device certification.'
    exit_code=$noraProcess.ExitCode
    forced_termination=$noraForced
    elapsed_ms=$noraClock.ElapsedMilliseconds
 }
 $noraEncoded=$noraReport | ConvertTo-Json -Depth 5
 [IO.File]::WriteAllText((Join-Path $noraOutput 'measurement.json'),$noraEncoded)
 $noraEncoded
 if($noraProcess.ExitCode -ne 0 -or $noraForced){throw 'NORA validation process failed; this is not a successful benchmark'}
}finally{$noraProcess.Dispose()}
