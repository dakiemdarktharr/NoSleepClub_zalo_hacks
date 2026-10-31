#Requires -Version 7.0
[CmdletBinding()]
param(
    [ValidateSet('Test','Check','Clippy','Release','Format')]
    [string]$Action='Test',
    [switch]$CoreOnly
)

$ErrorActionPreference='Stop'
$noraRepositoryRoot=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$noraLocalCargo=Join-Path $noraRepositoryRoot '.tools\cargo\bin\cargo.exe'
if (Test-Path -LiteralPath $noraLocalCargo) {
    $env:CARGO_HOME=Join-Path $noraRepositoryRoot '.tools\cargo'
    $env:RUSTUP_HOME=Join-Path $noraRepositoryRoot '.tools\rustup'
    $env:PATH=(Split-Path $noraLocalCargo -Parent) + [IO.Path]::PathSeparator + $env:PATH
}
if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install the pinned Rust toolchain first; see rust-toolchain.toml and https://rust-lang.org/tools/install/' }

Push-Location $noraRepositoryRoot
try {
    $noraTarget=@()
    if ($IsWindows -and -not $CoreOnly -and $Action -ne 'Format') {
        $noraVswhereCandidates=@(
            (Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'),
            (Join-Path $env:ProgramFiles 'Microsoft Visual Studio\Installer\vswhere.exe')
        )
        $noraVswhere=$noraVswhereCandidates | Where-Object { Test-Path -LiteralPath $_ } | Select-Object -First 1
        if ($noraVswhere) {
            $noraVisualStudio=(& $noraVswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath)
            if ($noraVisualStudio) {
                & (Join-Path $noraVisualStudio 'Common7\Tools\Launch-VsDevShell.ps1') -Arch amd64 -HostArch amd64 -SkipAutomaticLocation
                $noraBundledNinja=Join-Path $noraVisualStudio 'Common7\IDE\CommonExtensions\Microsoft\CMake\Ninja'
                if (Test-Path -LiteralPath (Join-Path $noraBundledNinja 'ninja.exe')) { $env:PATH=$noraBundledNinja + ';' + $env:PATH }
            }
        }
        if (-not (Get-Command cl -ErrorAction SilentlyContinue)) { throw 'Windows native builds require installed MSVC C++ tools. This script does not install system components.' }
        if (Get-Command ninja -ErrorAction SilentlyContinue) { $env:CMAKE_GENERATOR='Ninja' }
        $noraLlvmDir=Join-Path $env:ProgramFiles 'LLVM\bin'
        if (Test-Path -LiteralPath (Join-Path $noraLlvmDir 'libclang.dll')) { $env:LIBCLANG_PATH=$noraLlvmDir }
        $noraTarget=@('--target','x86_64-pc-windows-msvc')
        # Add the target to a GNU-hosted workspace toolchain too. This changes
        # the selected toolchain only, never system PATH or Visual Studio.
        & rustup target add x86_64-pc-windows-msvc
        if ($LASTEXITCODE -ne 0) { throw 'Rust MSVC target unavailable' }
    }
    $env:GGML_NATIVE='OFF'
    if ($IsWindows -and -not $CoreOnly -and $Action -ne 'Format') {
        # The pinned sys build forwards GGML_* but does not track every flag as
        # a rerun dependency. Retire a stale scalar/native cache after upgrades.
        $noraTargetRoot=[IO.Path]::GetFullPath((Join-Path $noraRepositoryRoot 'target'))
        $noraStaleProfiles=@()
        foreach ($noraProfile in @('debug','release')) {
            $noraNativeBuild=Join-Path $noraTargetRoot "x86_64-pc-windows-msvc/$noraProfile/build"
            if (Test-Path -LiteralPath $noraNativeBuild) {
                foreach ($noraBuildDir in Get-ChildItem -LiteralPath $noraNativeBuild -Directory -Filter 'whisper-rs-sys-*') {
                    $noraCache=Join-Path $noraBuildDir.FullName 'out/build/CMakeCache.txt'
                    if ((Test-Path -LiteralPath $noraCache) -and
                        ((Select-String -LiteralPath $noraCache -Quiet -Pattern '^GGML_AVX2:BOOL=OFF$') -or
                         (Select-String -LiteralPath $noraCache -Quiet -Pattern '^GGML_NATIVE:BOOL=ON$'))) { $noraStaleProfiles+=$noraProfile; break }
                }
            }
        }
        if ($noraStaleProfiles.Count -gt 0) {
            if (-not $noraTargetRoot.StartsWith($noraRepositoryRoot+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase) -or
                ((Get-Item -LiteralPath $noraTargetRoot).Attributes -band [IO.FileAttributes]::ReparsePoint)) { throw 'Refusing clean outside ordinary workspace target' }
            foreach($noraStaleProfile in $noraStaleProfiles){
                $noraCleanArgs=@('clean','-p','whisper-rs-sys','--target','x86_64-pc-windows-msvc')
                if($noraStaleProfile -eq 'release'){$noraCleanArgs+='--release'}
                & cargo @noraCleanArgs
                if ($LASTEXITCODE -ne 0) { throw 'Could not invalidate stale native profile' }
            }
        }
    }
    [string[]]$noraFeatures=@()
    if (-not $CoreOnly) { $noraFeatures+= '--all-features' }
    switch ($Action) {
        'Format' { & cargo fmt --all -- --check }
        'Test' { & cargo test --locked --workspace @noraFeatures @noraTarget }
        'Check' { & cargo check --locked --workspace --all-targets @noraFeatures @noraTarget }
        'Clippy' { & cargo clippy --locked --workspace --all-targets @noraFeatures @noraTarget -- -D warnings }
        'Release' { & cargo build --locked --release --jobs 1 -p nora-cli -p nora-app @noraFeatures @noraTarget }
    }
    if ($LASTEXITCODE -ne 0) { throw "Rust $Action failed" }
} finally { Pop-Location }
