$ErrorActionPreference = 'Stop'
# Use this engine's bundled modules when pwsh starts powershell.exe through Node.
# https://learn.microsoft.com/powershell/module/microsoft.powershell.core/about/about_psmodulepath
$env:PSModulePath = [IO.Path]::Combine($PSHOME, 'Modules')
$manifest = Get-Content -LiteralPath (Join-Path $PSScriptRoot 'FILES.json') -Raw | ConvertFrom-Json
$files = @(Get-ChildItem -LiteralPath $PSScriptRoot -Recurse -File)
foreach ($file in $files) {
  if (($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -ne 0) { throw 'Payload link rejected.' }
  $relative = $file.FullName.Substring($PSScriptRoot.Length + 1).Replace('\', '/')
  if ($relative -in @('FILES.json', 'Uninstall.exe')) { continue }
  $expected = $manifest.PSObject.Properties[$relative].Value
  if (!$expected -or (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash.ToLowerInvariant() -ne $expected) {
    throw 'Payload integrity check failed.'
  }
}
foreach ($entry in $manifest.PSObject.Properties) {
  if (!(Test-Path -LiteralPath (Join-Path $PSScriptRoot $entry.Name) -PathType Leaf)) { throw 'Payload file missing.' }
}
