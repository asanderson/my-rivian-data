param([Parameter(Mandatory=$true)][string]$Installer, [Parameter(Mandatory=$true)][string]$Version, [Parameter(Mandatory=$true)][string]$Revision, [ValidateRange(1,600)][int]$TimeoutSeconds = 120)
$ErrorActionPreference = 'Stop'

function Invoke-SilentProcess([string]$FilePath, [string]$Stage) {
  Write-Host "Starting: $Stage (deadline: $TimeoutSeconds seconds)"
  $process = Start-Process -FilePath $FilePath -ArgumentList '/S' -PassThru
  try {
    # Retain the handle so even a quickly exiting process has an exit code.
    $null = $process.Handle
    if (!$process.WaitForExit($TimeoutSeconds * 1000)) {
      & "$env:SystemRoot\System32\taskkill.exe" /PID $process.Id /T /F | Out-Null
      throw "$Stage timed out after $TimeoutSeconds seconds. The installer process tree was terminated."
    }
    Write-Host "Finished: $Stage (exit code $($process.ExitCode))"
    return $process.ExitCode
  } finally { $process.Dispose() }
}

$installed = Join-Path $env:LOCALAPPDATA "My Rivian Data\versions\$Version-$Revision"
if (Test-Path -LiteralPath $installed) { throw 'Installer check requires a clean test profile.' }
$diagnostic = Join-Path $env:TEMP "my-rivian-data-$Version-$Revision-verify.log"
Remove-Item -LiteralPath $diagnostic -Force -ErrorAction SilentlyContinue
Write-Host 'PowerShell execution policies (read-only):'
Get-ExecutionPolicy -List | Format-Table -AutoSize | Out-String | Write-Host
$exitCode = Invoke-SilentProcess $Installer 'Silent installation'
if ($exitCode -ne 0) {
  if (Test-Path -LiteralPath $diagnostic) { Get-Content -LiteralPath $diagnostic | Write-Host }
  throw "Silent installation failed (exit code $exitCode; 10 = existing build, 11 = payload verification)."
}
Write-Host 'Verifying installed payload'
& (Join-Path $installed 'verify.ps1')
Write-Host 'Checking installed executable version'
if ((& (Join-Path $installed 'my-rivian-data.exe') --version) -ne "My Rivian Data $Version") { throw 'Installed executable did not launch.' }
Write-Host 'Checking Start menu shortcut target'
$shortcut = Join-Path ([Environment]::GetFolderPath('StartMenu')) 'Programs\My Rivian Data\My Rivian Data.lnk'
$shell = New-Object -ComObject WScript.Shell
if ($shell.CreateShortcut($shortcut).TargetPath -ne (Join-Path $installed 'my-rivian-data.exe')) { throw 'Click-launch shortcut target differs.' }
$exitCode = Invoke-SilentProcess (Join-Path $installed 'Uninstall.exe') 'Silent uninstallation'
if ($exitCode -ne 0) { throw "Silent uninstallation failed (exit code $exitCode; 12 = payload still in use)." }
# NSIS relaunches its uninstaller from a temporary copy. The initial process can
# exit first, so validate completion by waiting for the actual payload removal.
Write-Host 'Waiting for uninstaller payload removal'
$deadline = [DateTime]::UtcNow.AddSeconds($TimeoutSeconds)
while ((Test-Path -LiteralPath $installed) -and [DateTime]::UtcNow -lt $deadline) { Start-Sleep -Milliseconds 100 }
if (Test-Path -LiteralPath $installed) { throw 'Uninstaller left the version payload.' }
if (Test-Path -LiteralPath $shortcut) { throw 'Uninstaller left the current shortcut.' }
Write-Output 'Per-user install, payload verification, shortcut target, executable and uninstall passed.'
