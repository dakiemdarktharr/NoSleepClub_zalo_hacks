#Requires -Version 7.0
[CmdletBinding()]
param([Parameter(Mandatory)][string]$Destination,[switch]$VerifyOnly)
$ErrorActionPreference='Stop'
$noraCatalogRoot=Join-Path $PSScriptRoot '../assets/manifests'
$noraAssetRoot=[IO.Path]::GetFullPath($Destination)
[IO.Directory]::CreateDirectory($noraAssetRoot) | Out-Null
if((Get-Item -LiteralPath $noraAssetRoot).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Asset root must be a local directory, not a link'}
$noraHandler=[Net.Http.HttpClientHandler]::new()
$noraHandler.UseProxy=$false
$noraHttp=[Net.Http.HttpClient]::new($noraHandler)
$noraHttp.Timeout=[TimeSpan]::FromMinutes(10)
$noraHttp.DefaultRequestHeaders.UserAgent.ParseAdd('NORA-Asset-Setup/0.1')
function Get-NoraAssetPath([string]$Relative){
    if($Relative -match '(^|/)\.\.?(/|$)' -or $Relative.Contains('\') -or $Relative.Contains(':')){throw 'Invalid catalog path'}
    $noraPath=[IO.Path]::GetFullPath((Join-Path $noraAssetRoot $Relative))
    if(-not $noraPath.StartsWith($noraAssetRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Asset path escaped destination'}
    $noraParent=[IO.Path]::GetDirectoryName($noraPath)
    [IO.Directory]::CreateDirectory($noraParent) | Out-Null
    for($noraCheck=$noraParent;$noraCheck.Length -ge $noraAssetRoot.Length;$noraCheck=[IO.Path]::GetDirectoryName($noraCheck)){
        if((Get-Item -LiteralPath $noraCheck).Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Refusing linked asset directory'}
        if($noraCheck -eq $noraAssetRoot){break}
    }
    return $noraPath
}
function Test-NoraPinnedFile([string]$Path,[long]$Bytes,[string]$Sha256){
    if(-not [IO.File]::Exists($Path)){return $false}
    $noraFile=Get-Item -LiteralPath $Path
    if($noraFile.Attributes -band [IO.FileAttributes]::ReparsePoint){throw 'Refusing linked asset file'}
    if($Bytes -gt 0 -and $noraFile.Length -ne $Bytes){return $false}
    return (Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash -eq $Sha256
}
function Get-NoraPinnedFile([uri]$Uri,[string]$Path,[long]$Bytes,[string]$Sha256,[long]$MaximumBytes){
    if(Test-NoraPinnedFile $Path $Bytes $Sha256){return}
    if([IO.File]::Exists($Path)){throw 'Existing asset has a different hash; it was left untouched'}
    if($VerifyOnly){throw 'Required verified asset is missing'}
    if($Uri.Scheme -ne 'https'){throw 'Asset source must use HTTPS'}
    $noraTemporary=$Path+'.partial-'+[guid]::NewGuid().ToString('N')
    $noraResponse=$null;$noraInput=$null;$noraOutput=$null
    $noraTimeout=[Threading.CancellationTokenSource]::new([TimeSpan]::FromMinutes(10))
    try{
        $noraResponse=$noraHttp.GetAsync($Uri,[Net.Http.HttpCompletionOption]::ResponseHeadersRead,$noraTimeout.Token).GetAwaiter().GetResult()
        $noraResponse.EnsureSuccessStatusCode() | Out-Null
        if($noraResponse.RequestMessage.RequestUri.Scheme -ne 'https' -or $noraResponse.Content.Headers.ContentLength -gt $MaximumBytes){throw 'Unsafe or oversized asset response'}
        $noraInput=$noraResponse.Content.ReadAsStreamAsync($noraTimeout.Token).GetAwaiter().GetResult()
        $noraOutput=[IO.File]::Open($noraTemporary,[IO.FileMode]::CreateNew,[IO.FileAccess]::Write,[IO.FileShare]::None)
        $noraBuffer=[byte[]]::new(65536);$noraReadBytes=0L
        while(($noraCount=$noraInput.ReadAsync($noraBuffer,0,$noraBuffer.Length,$noraTimeout.Token).GetAwaiter().GetResult()) -gt 0){$noraReadBytes+=$noraCount;if($noraReadBytes -gt $MaximumBytes){throw 'Asset exceeded byte limit'};$noraOutput.Write($noraBuffer,0,$noraCount)}
        $noraOutput.Flush($true);$noraOutput.Dispose();$noraOutput=$null
        if(-not (Test-NoraPinnedFile $noraTemporary $Bytes $Sha256)){throw 'Asset checksum mismatch'}
        [IO.File]::Move($noraTemporary,$Path,$false)
    }finally{if($noraOutput){$noraOutput.Dispose()};if($noraInput){$noraInput.Dispose()};if($noraResponse){$noraResponse.Dispose()};$noraTimeout.Dispose();if([IO.File]::Exists($noraTemporary)){[IO.File]::Delete($noraTemporary)}}
}
$noraPacks=@(
 @{manifest='vieneu-v3-turbo-fp32.json';folder='vieneu'},
 @{manifest='silero-vad.json';folder='silero'},
 @{manifest='sea-g2p.json';folder='g2p'},
 @{manifest='whisper-large-v3-turbo-q5_0.json';folder='whisper'},
 @{manifest='onnxruntime-win-x64-cpu.json';folder='ort'}
)
$noraTotal=0L;$noraFiles=0
try{
 foreach($noraPack in $noraPacks){
    $noraCatalog=Get-Content -LiteralPath (Join-Path $noraCatalogRoot $noraPack.manifest) -Raw | ConvertFrom-Json
    if($noraCatalog.schema_version -ne 1 -or $noraCatalog.files.Count -gt 64){throw 'Invalid asset catalog'}
    foreach($noraEntry in $noraCatalog.files){
        $noraTotal+=[long]$noraEntry.bytes;if($noraTotal -gt 15000000000){throw 'Asset budget exceeded'}
        $noraOutputPath=Get-NoraAssetPath ($noraPack.folder+'/'+$noraEntry.path)
        if(Test-NoraPinnedFile $noraOutputPath $noraEntry.bytes $noraEntry.sha256){$noraFiles++;continue}
        if($noraPack.folder -eq 'ort'){
            if($VerifyOnly){throw 'Verified ONNX Runtime DLL is missing'}
            if([IO.File]::Exists($noraOutputPath)){throw 'Existing ONNX Runtime DLL differs; left untouched'}
            $noraZip=Get-NoraAssetPath ('ort/runtime-'+[guid]::NewGuid().ToString('N')+'.zip')
            try{
                Get-NoraPinnedFile 'https://github.com/microsoft/onnxruntime/releases/download/v1.31.1/onnxruntime-win-x64-1.31.1.zip' $noraZip 0 '5869e26bb25314375f2d5ef1877e3ea06b1543d6c710e6e4a1ec2f7177ab4b2c' 128000000
                $noraArchive=[IO.Compression.ZipFile]::OpenRead($noraZip)
                try{
                    $noraMatches=@($noraArchive.Entries | Where-Object {$_.FullName.EndsWith('/lib/onnxruntime.dll',[StringComparison]::Ordinal)})
                    if($noraMatches.Count -ne 1 -or $noraMatches[0].Length -ne $noraEntry.bytes){throw 'Unexpected runtime archive contents'}
                    $noraDllTemporary=$noraOutputPath+'.partial-'+[guid]::NewGuid().ToString('N')
                    try{
                        $noraSource=$noraMatches[0].Open();$noraTarget=[IO.File]::Open($noraDllTemporary,[IO.FileMode]::CreateNew)
                        try{$noraSource.CopyTo($noraTarget,65536);$noraTarget.Flush($true)}finally{$noraTarget.Dispose();$noraSource.Dispose()}
                        if(-not (Test-NoraPinnedFile $noraDllTemporary $noraEntry.bytes $noraEntry.sha256)){throw 'Extracted runtime hash mismatch'}
                        [IO.File]::Move($noraDllTemporary,$noraOutputPath,$false)
                    }finally{if([IO.File]::Exists($noraDllTemporary)){[IO.File]::Delete($noraDllTemporary)}}
                }finally{$noraArchive.Dispose()}
            }finally{if([IO.File]::Exists($noraZip)){[IO.File]::Delete($noraZip)}}
        }else{
            $noraRemote=switch($noraPack.folder){
                'vieneu'{if($noraEntry.path.StartsWith('codec/')){'https://huggingface.co/OpenMOSS-Team/MOSS-Audio-Tokenizer-Nano-ONNX/resolve/ceff0d0749bfb3fa2d61149794ec6feef0d1e1ae/'+$noraEntry.path.Substring(6)}elseif($noraEntry.path -eq 'voices_v3_turbo.json'){'https://raw.githubusercontent.com/pnnbao97/VieNeu-TTS/eecd615d04e3d0e4b9c8330af66306da789b8f8d/src/vieneu/assets/voices_v3_turbo.json'}else{'https://huggingface.co/pnnbao-ump/VieNeu-TTS-v3-Turbo/resolve/61b85e3d937fbbacb387714180e8182823512523/onnx_update/'+$noraEntry.path}}
                'silero'{'https://raw.githubusercontent.com/snakers4/silero-vad/bfdc0193023f121ea5b3cc7b176dbed570a68a59/src/silero_vad/data/silero_vad.onnx'}
                'g2p'{'https://raw.githubusercontent.com/pnnbao97/sea-g2p/e825173f235d08ea19315b2b279fb11153b44cea/python/sea_g2p/sea_g2p.bin'}
                'whisper'{'https://huggingface.co/ggerganov/whisper.cpp/resolve/5359861c739e955e79d9a303bcbc70fb988958b1/'+$noraEntry.path}
            }
            Get-NoraPinnedFile $noraRemote $noraOutputPath $noraEntry.bytes $noraEntry.sha256 $noraEntry.bytes
        }
        if(-not (Test-NoraPinnedFile $noraOutputPath $noraEntry.bytes $noraEntry.sha256)){throw 'Final asset verification failed'}
        $noraFiles++
    }
 }
 [pscustomobject]@{verified_files=$noraFiles;logical_bytes=$noraTotal;publisher_authenticated=$false;hub_code_executed=$false} | ConvertTo-Json -Compress
}finally{$noraHttp.Dispose()}
