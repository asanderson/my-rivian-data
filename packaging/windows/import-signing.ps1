$ErrorActionPreference = 'Stop'
$password = ConvertTo-SecureString -String $env:RIVIAN_WINDOWS_PFX_PASSWORD -AsPlainText -Force
$certificate = Import-PfxCertificate -FilePath $env:RIVIAN_WINDOWS_PFX_PATH -CertStoreLocation Cert:\CurrentUser\My -Password $password
if (!$certificate.HasPrivateKey -or !$certificate.EnhancedKeyUsageList.ObjectId.Contains('1.3.6.1.5.5.7.3.3')) {
  throw 'Code signing certificate with private key required.'
}
Write-Output $certificate.Thumbprint
