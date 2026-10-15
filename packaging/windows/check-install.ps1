param([Parameter(Mandatory=$true)][string]$Installer, [Parameter(Mandatory=$true)][string]$Version, [Parameter(Mandatory=$true)][string]$Revision)
$ErrorActionPreference = 'Stop'
$installed = Join-Path $env:LOCALAPPDATA "My Rivian Data\versions\$Version-$Revision"
if (Test-Path -LiteralPath $installed) { throw 'Installer check requires a clean test profile.' }
$process = Start-Process -FilePath $Installer -ArgumentList '/S' -Wait -PassThru
if ($process.ExitCode -ne 0) { throw 'Silent installation failed.' }
& (Join-Path $installed 'verify.ps1')
if ((& (Join-Path $installed 'my-rivian-data.exe') --version) -ne "My Rivian Data $Version") { throw 'Installed executable did not launch.' }
$shortcut = Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs\My Rivian Data\My Rivian Data.lnk'
$shell = New-Object -ComObject WScript.Shell
if ($shell.CreateShortcut($shortcut).TargetPath -ne (Join-Path $installed 'my-rivian-data.exe')) { throw 'Click-launch shortcut target differs.' }
$process = Start-Process -FilePath (Join-Path $installed 'Uninstall.exe') -ArgumentList '/S' -Wait -PassThru
for ($attempt = 0; $attempt -lt 50 -and (Test-Path -LiteralPath $installed); $attempt++) { Start-Sleep -Milliseconds 100 }
if (Test-Path -LiteralPath $installed) { throw 'Uninstaller left the version payload.' }
if (Test-Path -LiteralPath $shortcut) { throw 'Uninstaller left the current shortcut.' }
Write-Output 'Per-user install, payload verification, shortcut target, executable and uninstall passed.'
