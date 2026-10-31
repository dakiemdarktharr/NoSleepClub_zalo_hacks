#Requires -Version 7.0
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$Executable,
    [string[]]$ArgumentList=@(),
    [string]$OutputJson,
    [ValidateRange(1,1000)][int]$SamplingMilliseconds=5,
    [ValidateRange(1,3600)][int]$TimeoutSeconds=300
)

$ErrorActionPreference='Stop'
$noraStartInfo=[Diagnostics.ProcessStartInfo]::new()
$noraStartInfo.FileName=$Executable
foreach ($noraArgument in $ArgumentList) { $noraStartInfo.ArgumentList.Add($noraArgument) }
$noraStartInfo.UseShellExecute=$false
$noraStartInfo.CreateNoWindow=$true
$noraStartInfo.RedirectStandardOutput=$true
$noraStartInfo.RedirectStandardError=$true
$noraClock=[Diagnostics.Stopwatch]::StartNew()
$noraMeasuredProcess=[Diagnostics.Process]::Start($noraStartInfo)
$noraStdoutTask=$noraMeasuredProcess.StandardOutput.ReadToEndAsync()
$noraStderrTask=$noraMeasuredProcess.StandardError.ReadToEndAsync()
$noraPeakWorkingSet=0L
$noraPeakCommit=0L
$noraPrivateSamples=0L
$noraSamples=0
$noraTimedOut=$false
try {
    while (-not $noraMeasuredProcess.HasExited) {
        try {
            $noraMeasuredProcess.Refresh()
            $noraPeakWorkingSet=[Math]::Max($noraPeakWorkingSet,$noraMeasuredProcess.PeakWorkingSet64)
            $noraPeakCommit=[Math]::Max($noraPeakCommit,$noraMeasuredProcess.PeakPagedMemorySize64)
            $noraPrivateSamples=[Math]::Max($noraPrivateSamples,$noraMeasuredProcess.PrivateMemorySize64)
            $noraSamples++
        } catch [ComponentModel.Win32Exception] { } catch [InvalidOperationException] { }
        if ($noraClock.Elapsed.TotalSeconds -gt $TimeoutSeconds) {
            $noraTimedOut=$true
            $noraMeasuredProcess.Kill($true)
            break
        }
        Start-Sleep -Milliseconds $SamplingMilliseconds
    }
    $noraMeasuredProcess.WaitForExit()
    $noraClock.Stop()
    $noraStdout=$noraStdoutTask.GetAwaiter().GetResult()
    $noraStderr=$noraStderrTask.GetAwaiter().GetResult()
    $noraReport=[ordered]@{
        scope='One process; children/GPU are not metered. Never use this alone to certify complete NORA hardware requirements.'
        executable=[IO.Path]::GetFileName($Executable)
        wall_ms=$noraClock.Elapsed.TotalMilliseconds
        sampled_peak_working_set_bytes=$noraPeakWorkingSet
        sampled_os_peak_process_commit_charge_bytes=$noraPeakCommit
        sampled_peak_private_bytes=$noraPrivateSamples
        sampling_interval_ms=$SamplingMilliseconds
        samples=$noraSamples
        valid_memory_sample=($noraSamples -gt 0 -and $noraPeakWorkingSet -gt 0)
        exit_code=$noraMeasuredProcess.ExitCode
        timed_out=$noraTimedOut
        stdout=$noraStdout
        stderr=$noraStderr
    }
    $noraEncoded=$noraReport | ConvertTo-Json -Depth 6
    if ($OutputJson) {
        if (Test-Path -LiteralPath $OutputJson) { throw 'Measurement output exists; choose a new path.' }
        [IO.File]::WriteAllText([IO.Path]::GetFullPath($OutputJson),$noraEncoded,[Text.UTF8Encoding]::new($false))
    }
    $noraEncoded
    if ($noraReport.exit_code -ne 0 -or $noraTimedOut) { throw 'Measured process failed or timed out; report is not a successful benchmark.' }
} finally { $noraMeasuredProcess.Dispose() }
